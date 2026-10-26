//! Explicit Linux systemd user integration; never changes lingering or exports secrets.
use crate::{cli::AutostartAction, profile::Profile};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn safe(path: &Path, dir: bool) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() || (if dir { !m.is_dir() } else { !m.is_file() }) => {
            bail!("Unsafe autostart path: {}", path.display())
        }
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn text(path: &Path) -> Result<&str> {
    let s = path.to_str().context("Autostart requires UTF-8 paths")?;
    if s.chars().any(|c| c.is_control()) {
        bail!("Autostart paths cannot contain control characters");
    }
    Ok(s)
}

fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

struct Unit {
    name: String,
    path: PathBuf,
    identity: String,
}
impl Unit {
    fn new(p: &Profile) -> Result<Self> {
        let roots = [&p.config, &p.data, &p.state, &p.cache]
            .map(|p| text(p))
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        let identity = serde_json::to_string(&roots)?;
        let name = format!(
            "opencode-profile-{}-{:016x}.service",
            p.name,
            hash(identity.as_bytes())
        );
        Ok(Self {
            path: p.config.join("autostart").join(&name),
            name,
            identity,
        })
    }
    fn owned(&self) -> Result<bool> {
        safe(self.path.parent().unwrap(), true)?;
        if !safe(&self.path, false)? {
            return Ok(false);
        }
        let content = fs::read_to_string(&self.path)?;
        let (header, body) = content
            .split_once('\n')
            .context("Unmanaged autostart unit")?;
        if header
            != format!(
                "# opencode-profile {} {:016x}",
                self.identity,
                hash(body.as_bytes())
            )
        {
            bail!("Autostart unit ownership or content mismatch; refusing adoption");
        }
        Ok(true)
    }
    fn write(&self, p: &Profile) -> Result<()> {
        self.owned()?;
        let exe = env::current_exe().context("Cannot locate launcher executable")?;
        if !exe.is_absolute() {
            bail!("Launcher executable must be absolute");
        }
        let mut body = String::from("[Unit]\nDescription=OpenCode profile service\n\n[Service]\nType=oneshot\nRemainAfterExit=yes\n");
        for key in ["HOME", "PATH"] {
            let value = env::var(key).with_context(|| format!("Autostart requires UTF-8 {key}"))?;
            body.push_str(&format!(
                "Environment={}\n",
                quote(&format!("{key}={value}"), false)?
            ));
        }
        for (key, root) in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
        ]
        .into_iter()
        .zip([&p.config, &p.data, &p.state, &p.cache])
        {
            let base = root
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .context("Invalid profile root")?;
            body.push_str(&format!(
                "Environment={}\n",
                quote(&format!("{key}={}", text(base)?), false)?
            ));
        }
        for key in [
            "OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS",
            "OPENCODE_ENABLE_EXA",
        ] {
            if let Some(value) = env::var_os(key) {
                let value = value.to_str().context("Autostart boolean must be UTF-8")?;
                if !matches!(value, "true" | "false" | "1" | "0") {
                    bail!("{key} must be a boolean for autostart");
                }
                body.push_str(&format!(
                    "Environment={}\n",
                    quote(&format!("{key}={value}"), false)?
                ));
            }
        }
        for (directive, action) in [("ExecStart", "start"), ("ExecStop", "stop")] {
            body.push_str(&format!(
                "{directive}={} service {} {action}\n",
                quote(text(&exe)?, true)?,
                quote(&p.name, true)?
            ));
        }
        body.push_str("\n[Install]\nWantedBy=default.target\n");
        let content = format!(
            "# opencode-profile {} {:016x}\n{body}",
            self.identity,
            hash(body.as_bytes())
        );
        let dir = self.path.parent().unwrap();
        fs::create_dir_all(dir)?;
        let temp = dir.join(format!(".unit-{}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .context("Cannot create private autostart unit")?;
        let result = (|| {
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

fn quote(s: &str, exec: bool) -> Result<String> {
    if s.chars().any(|c| c.is_control()) {
        bail!("Autostart values cannot contain control characters");
    }
    let s = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    Ok(format!(
        "\"{}\"",
        if exec { s.replace('$', "$$") } else { s }
    ))
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn systemd_escaping_distinguishes_exec_arguments_and_environment() {
        assert_eq!(quote("path % $ \"", true).unwrap(), "\"path %% $$ \\\"\"");
        assert_eq!(quote("KEY=% $ \"", false).unwrap(), "\"KEY=%% $ \\\"\"");
        assert!(quote("bad\nvalue", true).is_err());
    }
}

fn lock(p: &Profile) -> Result<File> {
    safe(p.state.parent().context("Invalid state root")?, true)?;
    let parent = p
        .state
        .parent()
        .and_then(Path::parent)
        .context("Invalid state root")?;
    safe(parent, true)?;
    let dir = parent.join("autostart-locks");
    safe(&dir, true)?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.lock", p.name));
    safe(&path, false)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.lock_exclusive()?;
    Ok(file)
}

fn systemctl(args: &[&str]) -> Result<Output> {
    if !cfg!(target_os = "linux") {
        bail!("Autostart requires Linux systemd user services");
    }
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .context("systemd user manager unavailable (systemctl required)")
}
fn control(args: &[&str]) -> Result<()> {
    if !systemctl(args)?.status.success() {
        bail!("systemd user operation '{}' failed; check your user manager (no sudo or linger is configured)", args[0]);
    }
    Ok(())
}

fn status(unit: &Unit) -> Result<&'static str> {
    let owned = unit.owned()?;
    // Query the manager even without a source file: unavailable is not disabled.
    let output = systemctl(&["is-enabled", &unit.name])?;
    let state = String::from_utf8_lossy(&output.stdout);
    match state.trim() {
        "enabled" | "enabled-runtime" if output.status.success() && owned => Ok("enabled"),
        "disabled" | "not-found" | "static" | "linked" | "linked-runtime" | "masked" => {
            Ok("disabled")
        }
        _ => Ok("unavailable"),
    }
}

pub(crate) fn snapshot(p: &Profile) -> &'static str {
    (|| {
        let _lock = lock(p)?;
        p.require_config()?;
        status(&Unit::new(p)?)
    })()
    .unwrap_or("unavailable")
}

pub(crate) fn operate(p: &Profile, action: AutostartAction) -> Result<i32> {
    let _lock = lock(p)?;
    p.require_config()?;
    let unit = Unit::new(p)?;
    match action {
        AutostartAction::Enable => {
            unit.owned()?;
            crate::service::validate(p)?;
            control(&["show-environment"])?;
            let existing = systemctl(&["show", "--property=FragmentPath", "--value", &unit.name])?;
            let fragment = String::from_utf8_lossy(&existing.stdout);
            if !fragment.trim().is_empty() && fragment.trim() != text(&unit.path)? {
                bail!("Systemd unit name is already owned by another source");
            }
            unit.write(p)?;
            if let Err(error) = control(&["enable", "--now", text(&unit.path)?]) {
                control(&["disable", &unit.name]).context(
                    "Enable failed and disabling partial autostart registration also failed",
                )?;
                return Err(error);
            }
            println!("Autostart '{}' enabled", p.name);
        }
        AutostartAction::Disable => {
            if unit.owned()? {
                control(&["disable", text(&unit.path)?])?;
            }
            println!("Autostart '{}' disabled; running service unchanged", p.name);
        }
        AutostartAction::Status => println!("Autostart '{}': {}", p.name, status(&unit)?),
    }
    Ok(0)
}

pub(crate) fn remove_profile(p: &Profile) -> Result<()> {
    let _lock = lock(p)?;
    safe(&p.config, true)?;
    let unit = Unit::new(p)?;
    if unit.owned()? {
        p.require_config()?;
        let output = systemctl(&["is-active", &unit.name])?;
        let state = String::from_utf8_lossy(&output.stdout);
        match (state.trim(), output.status.code()) {
            ("active" | "activating" | "deactivating" | "reloading" | "refreshing", Some(0 | 3)) => {
                control(&["stop", &unit.name])?;
            }
            ("inactive" | "failed" | "unknown", Some(3 | 4)) => (),
            _ => bail!("Cannot determine autostart unit activity; profile preserved (check the systemd user manager)"),
        }
        control(&["disable", text(&unit.path)?])?;
        fs::remove_file(&unit.path)?;
        control(&["daemon-reload"])?;
    }
    crate::service::remove(p)
}
