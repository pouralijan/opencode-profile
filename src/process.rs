use crate::profile::Profile;
use anyhow::{bail, Context, Result};
use std::fs;
use std::process::{Command, Stdio};

fn server_position(args: &[String]) -> Result<Option<usize>> {
    // A caller-selected server, including --server=URL, would defeat profile isolation.
    if args.iter().any(|arg| {
        arg == "--server"
            || arg.starts_with("--server=")
            || arg == "--no-standalone"
            || arg.starts_with("--no-standalone=")
            || arg == "--standalone"
            || arg.starts_with("--standalone=")
    }) {
        bail!("Do not pass --server or --standalone; the launcher manages service isolation");
    }
    let route = args.first().map(String::as_str);
    let position = match route {
        None => Some(0), // Interactive root command.
        Some("run" | "mini" | "models" | "stats" | "api" | "reload") => Some(1),
        Some("auth")
            if matches!(
                args.get(1).map(String::as_str),
                Some("list" | "login" | "logout" | "export" | "import" | "switch")
            ) =>
        {
            Some(2)
        }
        Some("session")
            if matches!(
                args.get(1).map(String::as_str),
                Some("list" | "delete" | "export" | "import")
            ) =>
        {
            Some(2)
        }
        Some("debug") if args.get(1).map(String::as_str) == Some("paths") => None,
        Some(arg) if arg.starts_with('-') && root_options_only(&args) => Some(0),
        _ => bail!(
            "Unsupported OpenCode command: cannot guarantee private service; use a supported route"
        ),
    };
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "--version" | "-v") {
        return Ok(None);
    }
    Ok(position)
}

fn root_options_only(args: &[String]) -> bool {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if matches!(arg, "--prompt" | "--session" | "-s" | "--log-level") {
            i += 1;
            if i == args.len() {
                return false;
            }
        } else if !matches!(
            arg,
            "--auto" | "--continue" | "-c" | "--help" | "-h" | "--version" | "-v" | "--print-logs"
        ) && !["--prompt=", "--session=", "--log-level="]
            .iter()
            .any(|prefix| arg.starts_with(prefix))
        {
            return false;
        }
        i += 1;
    }
    true
}

pub(crate) fn run(profile: &Profile, args: Vec<String>) -> Result<i32> {
    profile.require_config()?;
    let position = server_position(&args)?;
    let mut args = args;
    let mut command = if let Some(position) = position {
        let (command, url) = crate::service::prepare_client(profile)?;
        args.splice(position..position, ["--server".to_owned(), url]);
        command
    } else {
        profile_command(profile, "opencode")?
    };
    command.args(args);
    launch(&mut command).context("Failed to launch opencode; is it installed and in PATH?")
}

pub(crate) fn exec(profile: &Profile, args: Vec<String>) -> Result<i32> {
    let mut command = if args.len() == 1 {
        #[cfg(unix)]
        let (shell, flag) = ("sh", "-c");
        #[cfg(not(unix))]
        let (shell, flag) = ("cmd.exe", "/C");
        let mut command = profile_command(profile, shell)?;
        command.args([flag, &args[0]]);
        command
    } else {
        let mut command = profile_command(profile, &args[0])?;
        command.args(&args[1..]);
        command
    };
    launch(&mut command).context("Failed to execute command in profile environment")
}

pub(crate) fn profile_command(profile: &Profile, program: &str) -> Result<Command> {
    profile.require_config()?;
    for path in [
        &profile.data,
        &profile.state,
        &profile.cache,
        &profile.config.join(".xdg"),
    ] {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                bail!(
                    "Refusing non-directory or symlink profile path: {}",
                    path.display()
                );
            }
        }
    }
    fs::create_dir_all(&profile.data)?;
    fs::create_dir_all(&profile.state)?;
    fs::create_dir_all(&profile.cache)?;
    // XDG_CONFIG_HOME/opencode/cli.json holds CLI preferences, separate from OPENCODE_CONFIG_DIR.
    let xdg_config = profile.config.join(".xdg");
    fs::create_dir_all(xdg_config.join("opencode"))?;
    let mut command = Command::new(program);
    command
        .env("OPENCODE_CONFIG_DIR", &profile.config)
        .env("OPENCODE_PROFILE", &profile.name)
        .env("XDG_CONFIG_HOME", xdg_config)
        .env("XDG_DATA_HOME", &profile.data)
        .env("XDG_STATE_HOME", &profile.state)
        .env("XDG_CACHE_HOME", &profile.cache)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    Ok(command)
}

fn launch(command: &mut Command) -> Result<i32> {
    let status = command.status()?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
    }
    #[cfg(not(unix))]
    {
        Ok(status.code().unwrap_or(1))
    }
}
