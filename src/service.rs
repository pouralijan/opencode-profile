//! Ownership checks and serialization around OpenCode's native service manager.
use crate::{cli::ServiceAction, process::profile_command, profile::Profile};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            bail!("Unsafe service directory: {}", path.display())
        }
        Ok(_) => (),
        Err(e) if e.kind() == ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn regular(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
            bail!("Unsafe service file: {}", path.display())
        }
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn lock(profile: &Profile) -> Result<File> {
    directory(profile.state.parent().context("Invalid state root")?)?;
    let parent = profile
        .state
        .parent()
        .and_then(Path::parent)
        .context("Invalid state root")?;
    directory(parent)?;
    let dir = parent.join("service-locks");
    directory(&dir)?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.lock", profile.name));
    regular(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.lock_exclusive()?;
    Ok(file)
}

struct Managed {
    config: PathBuf,
    record: PathBuf,
    state_marker: PathBuf,
    config_marker: PathBuf,
    identity: Value,
}

impl Managed {
    fn inspect(profile: &Profile) -> Result<Self> {
        for path in [
            &profile.config,
            &profile.data,
            &profile.state,
            &profile.cache,
        ] {
            directory(path)?;
        }
        for path in [
            profile.config.join(".xdg"),
            profile.config.join(".xdg/opencode"),
            profile.state.join("opencode"),
        ] {
            directory(&path)?;
        }
        let this = Self {
            config: profile.config.join("service.json"),
            record: profile.state.join("opencode/service.json"),
            state_marker: profile.state.join("opencode-profile-service.json"),
            config_marker: profile.config.join("opencode-profile-service.json"),
            identity: json!({"version": 1, "config": profile.config, "data": profile.data, "state": profile.state, "cache": profile.cache}),
        };
        let state_owned = this.marker(&this.state_marker)?;
        let config_owned = this.marker(&this.config_marker)?;
        let record = regular(&this.record)?;
        let config = regular(&this.config)?;
        if (record || config) && !(state_owned && config_owned) {
            bail!("Unmanaged native service metadata exists; refusing adoption. Stop and remove it manually before using managed services");
        }
        if config {
            this.port()?;
        }
        if record {
            this.validate_record()?;
        }
        Ok(this)
    }

    fn marker(&self, path: &Path) -> Result<bool> {
        if !regular(path)? {
            return Ok(false);
        }
        let value: Value = serde_json::from_slice(&fs::read(path)?)
            .context("Malformed service ownership manifest")?;
        if value != self.identity {
            bail!(
                "Service ownership mismatch at {}; use the original four profile roots",
                path.display()
            );
        }
        Ok(true)
    }

    fn port(&self) -> Result<u16> {
        let value: Value = serde_json::from_slice(&fs::read(&self.config)?)
            .context("Malformed native service config")?;
        let object = value.as_object().context("Invalid native service config")?;
        let port = value
            .get("port")
            .and_then(Value::as_u64)
            .filter(|p| *p > 0 && *p <= u16::MAX as u64)
            .context("Native service port must be positive")? as u16;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "hostname" | "port" | "password"))
            || value.get("hostname").and_then(Value::as_str) != Some("127.0.0.1")
            || value
                .get("password")
                .is_some_and(|password| password.as_str().is_none_or(str::is_empty))
        {
            bail!("Unsafe native service config; only managed loopback hostname, port, and native password are allowed (no env overrides)");
        }
        Ok(port)
    }

    fn validate_record(&self) -> Result<()> {
        if !regular(&self.config)? {
            bail!("Native service registration has no managed config");
        }
        let value: Value = serde_json::from_slice(&fs::read(&self.record)?)
            .context("Malformed native service registration")?;
        let expected = format!("http://127.0.0.1:{}", self.port()?);
        if value.get("url").and_then(Value::as_str) != Some(expected.as_str()) {
            bail!("Native service registration URL does not match managed loopback config");
        }
        Ok(())
    }

    fn provision(&self) -> Result<()> {
        for marker in [&self.state_marker, &self.config_marker] {
            if !self.marker(marker)? {
                create_json(marker, &self.identity)?;
            }
        }
        if !regular(&self.config)? {
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            let port = listener.local_addr()?.port();
            drop(listener);
            create_json(
                &self.config,
                &json!({"hostname": "127.0.0.1", "port": port}),
            )?;
        }
        self.port()?;
        Ok(())
    }

    fn start(&self, profile: &Profile, quiet: bool) -> Result<()> {
        self.provision()?;
        let port = self.port()?;
        native(profile, "start", quiet).with_context(|| format!(
            "Could not start the profile service. Ensure OpenCode supports native service commands. If port {} is occupied, stop this profile service first; only after confirming {} is absent, remove {} and start again to allocate a new port",
            port, self.record.display(), self.config.display()
        ))?;
        if !regular(&self.record)? {
            bail!("Native service start did not create a registration");
        }
        self.validate_record()
    }

    fn stop(&self, profile: &Profile) -> Result<()> {
        if regular(&self.record)? {
            native(profile, "stop", false)?;
            if regular(&self.record)? {
                bail!("Native service stop left a registration; profile directories preserved");
            }
        }
        Ok(())
    }
}

fn create_json(path: &Path, value: &Value) -> Result<()> {
    fs::create_dir_all(path.parent().context("Missing metadata parent")?)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}

fn native(profile: &Profile, action: &str, quiet: bool) -> Result<()> {
    let mut command = profile_command(profile, "opencode")?;
    command.args(["service", action]);
    if quiet {
        command.stdout(Stdio::null());
    }
    let status = command
        .status()
        .context("Failed to launch native OpenCode service manager")?;
    if !status.success() {
        bail!("Native OpenCode service {action} failed; no fallback was attempted");
    }
    Ok(())
}

pub(crate) fn prepare_client(profile: &Profile) -> Result<(Command, String)> {
    let _lock = lock(profile)?;
    profile.require_config()?;
    let managed = Managed::inspect(profile)?;
    managed.start(profile, true)?;
    // Capture the validated endpoint and credentials while removal is excluded.
    // Explicit attachment cannot rediscover or resurrect a subsequently stopped service.
    managed.validate_record()?;
    let record: Value = serde_json::from_slice(&fs::read(&managed.record)?)
        .context("Malformed native service registration")?;
    let password = record
        .get("password")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .context("Native service registration has no password")?;
    let url = format!("http://127.0.0.1:{}", managed.port()?);
    let mut command = profile_command(profile, "opencode")?;
    command
        .env("OPENCODE_PASSWORD", password)
        .env("OPENCODE_SERVER_PASSWORD", password);
    Ok((command, url))
}

pub(crate) fn operate(profile: &Profile, action: ServiceAction) -> Result<i32> {
    if let ServiceAction::Autostart { action } = action {
        return crate::autostart::operate(profile, action);
    }
    let _lock = lock(profile)?;
    profile.require_config()?;
    let managed = Managed::inspect(profile)?;
    match action {
        ServiceAction::Autostart { .. } => unreachable!(),
        ServiceAction::Start => managed.start(profile, false)?,
        ServiceAction::Stop => managed.stop(profile)?,
        ServiceAction::Restart => {
            managed.stop(profile)?;
            managed.start(profile, false)?;
        }
        ServiceAction::Status => {
            if regular(&managed.record)? {
                native(profile, "status", false)?;
            } else {
                println!("Service '{}' is stopped", profile.name);
            }
        }
    }
    Ok(0)
}

pub(crate) fn snapshot(profile: &Profile) -> &'static str {
    snapshot_checked(profile).unwrap_or("unhealthy")
}

pub(crate) fn validate(profile: &Profile) -> Result<()> {
    let _lock = lock(profile)?;
    profile.require_config()?;
    Managed::inspect(profile)?;
    Ok(())
}

fn snapshot_checked(profile: &Profile) -> Result<&'static str> {
    let _lock = lock(profile)?;
    profile.require_config()?;
    let managed = Managed::inspect(profile)?;
    if !regular(&managed.record)? {
        return Ok("stopped");
    }
    let output = profile_command(profile, "opencode")?
        .args(["service", "status"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if !output.status.success() {
        return Ok("unhealthy");
    }
    let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
    if text.contains("not running") || text.contains("stopped") {
        Ok("stopped")
    } else if text.trim() == format!("http://127.0.0.1:{}", managed.port()?)
        || text.contains("running")
    {
        Ok("running")
    } else {
        Ok("unknown")
    }
}

pub(crate) fn remove(profile: &Profile) -> Result<()> {
    let _lock = lock(profile)?;
    if !profile.exists() {
        bail!("Profile '{}' not found", profile.name);
    }
    let managed = Managed::inspect(profile)?;
    managed.stop(profile)?;
    profile.remove()
}
