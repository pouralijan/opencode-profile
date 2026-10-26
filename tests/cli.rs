use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn invoke(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_opencode-profile"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .output()
        .unwrap()
}

fn config(home: &Path, name: &str) -> std::path::PathBuf {
    home.join("config/opencode-profile/profiles").join(name)
}

fn data(home: &Path, name: &str) -> std::path::PathBuf {
    home.join("data/opencode-profile/profiles").join(name)
}

#[cfg(unix)]
fn state(home: &Path, name: &str) -> std::path::PathBuf {
    home.join("state/opencode-profile/profiles").join(name)
}

#[cfg(unix)]
fn install_fake_opencode(home: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("opencode");
    fs::write(&executable, r##"#!/bin/sh
set -eu
record="$XDG_STATE_HOME/opencode/service.json"
if [ "${1:-}" = service ]; then
    mkdir -p "$XDG_STATE_HOME/opencode"
    printf '%s\n' "$*" >> "$XDG_STATE_HOME/fake-calls"
    case "$2" in
        start)
            [ ! -f "$XDG_STATE_HOME/fake-start-fail" ] || exit 19
            if [ ! -f "$record" ]; then
                count=0
                [ ! -f "$XDG_STATE_HOME/fake-starts" ] || read -r count < "$XDG_STATE_HOME/fake-starts"
                count=$((count + 1))
                printf '%s\n' "$count" > "$XDG_STATE_HOME/fake-starts"
                port=49374
                if [ -f "$OPENCODE_CONFIG_DIR/service.json" ]; then
                    port=$(sed -n 's/.*"port":[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$OPENCODE_CONFIG_DIR/service.json")
                fi
                printf '{"hostname":"127.0.0.1","port":%s,"password":"fake"}\n' "$port" > "$OPENCODE_CONFIG_DIR/service.json"
                printf '{"version":"2.1.3","pid":123,"id":"fake-%s","url":"http://127.0.0.1:%s","password":"fake"}\n' "$count" "$port" > "$record"
            fi
            ;;
        status)
            if [ -f "$record" ] && [ ! -f "$XDG_STATE_HOME/fake-stale" ]; then
                sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$record"
            else
                printf 'stopped\n'
            fi
            ;;
        stop)
            [ ! -f "$XDG_STATE_HOME/fake-stop-fail" ] || exit 17
            rm -f "$record"
            ;;
        *) exit 2 ;;
    esac
    exit 0
fi
if [ -f "$HOME/pause-client" ]; then
    printf 'CLIENT_READY\n'
    read -r release
fi
printf 'CONFIG=%s\nDATA=%s\nSTATE=%s\nCACHE=%s\nXDG_CONFIG=%s\n' "$OPENCODE_CONFIG_DIR" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_CONFIG_HOME"
local=0
case "${1:-}" in debug|--help|-h|--version|-v) local=1 ;; esac
attached=0
printed=0
while [ "$#" -gt 0 ]; do
    if [ "$1" = --server ]; then
        [ -f "$record" ] || exit 24
        [ "${OPENCODE_PASSWORD:-}" = fake ] || exit 25
        [ "${OPENCODE_SERVER_PASSWORD:-}" = fake ] || exit 25
        target=$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$record")
        [ "$2" = "$target" ] || exit 26
        printf '%s\n' "$2" >> "$XDG_STATE_HOME/fake-client-targets"
        attached=1
        shift 2
    else
        printf 'ARG=%s\n' "$1"
        printed=1
        shift
    fi
done
[ "$printed" = 1 ] || printf 'ARG=\n'
if [ "$attached" = 0 ] && [ "$local" = 0 ]; then
    "$0" service start
fi
exit 23
"##).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap())
}

#[cfg(unix)]
fn fake_command(home: &Path, path: &str, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-profile"));
    command
        .args(args)
        .env("PATH", path)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"));
    command
}

#[cfg(target_os = "linux")]
fn install_fake_systemctl(home: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let executable = home.join("bin/systemctl");
    fs::write(
        &executable,
        r##"#!/bin/sh
set -eu
[ "$1" = --user ] || exit 2
shift
action=$1
shift
mkdir -p "$HOME/fake-systemd"
printf '%s' "$action" >> "$HOME/fake-systemctl-calls"
for arg in "$@"; do printf ' <%s>' "$arg" >> "$HOME/fake-systemctl-calls"; done
printf '\n' >> "$HOME/fake-systemctl-calls"
if [ -f "$HOME/manager-unavailable" ]; then
    printf 'Failed to connect to bus: synthetic unavailable manager\n' >&2
    exit 1
fi
if [ -f "$HOME/fail-$action" ]; then exit 1; fi
target=''
now=0
for arg in "$@"; do
    case "$arg" in --now) now=1 ;; --*) ;; *) target=$arg ;; esac
done
unit=${target##*/}
store="$HOME/fake-systemd/$unit"
callback() {
    if [ -f "$HOME/simulate-unit-callback" ] && [ -f "$store.source" ]; then
        source=$(cat "$store.source")
        profile_dir=$(dirname "$(dirname "$source")")
        profile=${profile_dir##*/}
        launcher=$(cat "$HOME/test-launcher")
        "$launcher" service "$profile" "$1"
    fi
}
case "$action" in
    enable)
        [ -f "$target" ] || exit 2
        printf '%s\n' "$target" > "$store.source"
        : > "$store.enabled"
        if [ "$now" = 1 ]; then callback start; : > "$store.active"; fi
        ;;
    disable)
        if [ "$now" = 1 ]; then callback stop; rm -f "$store.active"; fi
        rm -f "$store.enabled"
        ;;
    stop) callback stop; rm -f "$store.active" ;;
    is-enabled)
        if [ -f "$store.enabled" ]; then printf 'enabled\n'; else printf 'disabled\n'; exit 1; fi
        ;;
    is-active)
        if [ -f "$store.active" ]; then printf 'active\n'; else printf 'inactive\n'; exit 3; fi
        ;;
    show-environment) printf 'PATH=/synthetic/bin\n' ;;
    show)
        if [ -f "$store.source" ] && { [ -f "$store.enabled" ] || [ -f "$store.active" ]; }; then
            cat "$store.source"
        fi
        ;;
    daemon-reload|link|reset-failed) ;;
    *) printf 'Unexpected fake systemctl action: %s\n' "$action" >&2; exit 2 ;;
esac
"##,
    )
    .unwrap();
    fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        home.join("test-launcher"),
        env!("CARGO_BIN_EXE_opencode-profile"),
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
fn unit_files(home: &Path, name: &str) -> Vec<std::path::PathBuf> {
    let dir = config(home, name).join("autostart");
    if !dir.exists() {
        return Vec::new();
    }
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "service"))
        .collect()
}

#[test]
fn help_documents_every_command_without_creating_profiles() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    for args in [vec!["--help"], vec![]] {
        let output = invoke(home, &args);
        let text = if args.is_empty() {
            assert!(!output.status.success());
            String::from_utf8(output.stderr).unwrap()
        } else {
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap()
        };
        for command in [
            "create", "list", "show", "run", "exec", "service", "services", "clone", "remove",
            "doctor",
        ] {
            assert!(text.contains(command), "Missing {command}: {text}");
        }
    }
    for command in [
        "create", "list", "show", "run", "exec", "service", "services", "clone", "remove", "doctor",
    ] {
        for args in [vec!["help", command], vec![command, "--help"]] {
            let output = invoke(home, &args);
            assert!(output.status.success(), "{args:?}: {:?}", output.stderr);
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("Usage:"), "{text}");
            if command == "exec" {
                assert!(
                    text.contains("sh -c") && text.contains("escape hatch"),
                    "{text}"
                );
            }
            if command == "run" {
                assert!(
                    text.contains("persistent") && text.contains("auth login"),
                    "{text}"
                );
            }
        }
    }
    let output = invoke(home, &["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("opencode-profile {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(!home.join("config").exists());
}

#[test]
fn exec_requires_a_command_and_an_existing_profile() {
    let dir = TempDir::new().unwrap();
    assert!(!invoke(dir.path(), &["exec", "missing", "echo unexpected"])
        .status
        .success());
    assert!(!invoke(dir.path(), &["exec", "hp"]).status.success());
    assert!(
        !invoke(dir.path(), &["exec", "../escape", "echo unexpected"])
            .status
            .success()
    );
}

#[cfg(unix)]
#[test]
fn exec_uses_profile_environment_and_preserves_output_arguments_and_status() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let script = home.join("fake installer.sh");
    fs::write(&script, r#"
printf '%s\n' "$OPENCODE_CONFIG_DIR" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$OPENCODE_PROFILE"
printf 'ARG=%s\n' "$@"
printf 'diagnostic\n' >&2
exit 23
"#).unwrap();

    for name in ["hp", "arman"] {
        assert!(invoke(home, &["create", name]).status.success());
        let shell_command = format!(
            "sh '{}' 'hello world' '--standalone' '--server=example'",
            script.display()
        );
        for args in [
            vec!["exec", name, shell_command.as_str()],
            vec![
                "exec",
                name,
                "--",
                "sh",
                script.to_str().unwrap(),
                "hello world",
                "--standalone",
                "--server=example",
            ],
        ] {
            let output = invoke(home, &args);
            assert_eq!(output.status.code(), Some(23));
            assert_eq!(output.stderr, b"diagnostic\n");
            let expected = format!(
                "{}\n{}\n{}\n{}\n{}\n{name}\nARG=hello world\nARG=--standalone\nARG=--server=example\n",
                config(home, name).display(),
                config(home, name).join(".xdg").display(),
                data(home, name).display(),
                home.join("state/opencode-profile/profiles").join(name).display(),
                home.join("cache/opencode-profile/profiles").join(name).display(),
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        }
    }
}

#[cfg(unix)]
#[test]
fn exec_rejects_symlinked_profile_paths() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(invoke(home, &["create", "hp"]).status.success());
    let outside = home.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, config(home, "hp").join(".xdg")).unwrap();
    let output = invoke(home, &["exec", "hp", "echo unexpected"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(fs::read_dir(outside).unwrap().next().is_none());
}

#[test]
fn inspection_commands_display_tables() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(invoke(home, &["list"]).stdout.is_empty());
    assert!(invoke(home, &["create", "demo"]).status.success());

    for (args, labels) in [
        (
            vec!["list"],
            vec!["◈ Profile", "● Status", "✓ ready", "demo"],
        ),
        (
            vec!["show", "demo"],
            vec![
                "Field",
                "Value",
                "CLI preferences",
                ".xdg/opencode/cli.json",
            ],
        ),
        (
            vec!["doctor"],
            vec![
                "Check",
                "Value",
                "Config root",
                "State root",
                "Cache root",
                "demo",
            ],
        ),
    ] {
        let output = invoke(home, &args);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            !text.contains('\x1b'),
            "Piped output must not contain ANSI codes: {text}"
        );
        assert!(
            text.contains('╭') && text.contains('│') && text.contains('╯'),
            "{text}"
        );
        for label in labels {
            assert!(text.contains(label), "Missing {label}: {text}");
        }
        assert!(text.contains("auth unverified"), "{text}");
    }
}

#[test]
fn create_validation_and_clone_excludes_secrets_and_data() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    for invalid in ["../escape", "-bad", "bad name", "é", "a/b", &"a".repeat(65)] {
        assert!(!invoke(home, &["create", invalid]).status.success());
    }
    assert!(invoke(home, &["create", "hp"]).status.success());
    assert!(config(home, "hp").join("opencode.json").exists());
    assert!(!invoke(home, &["create", "hp"]).status.success());
    fs::write(config(home, "hp").join("opencode.json"),
        r#"{"theme":"dark","model":"example/model","provider":{"secret":"private"},"other":"secret"}"#).unwrap();
    fs::write(config(home, "hp").join("auth.json"), "private").unwrap();
    fs::write(data(home, "hp").join("credential.txt"), "private").unwrap();
    assert!(invoke(home, &["clone", "hp", "arman"]).status.success());
    let cloned = fs::read_to_string(config(home, "arman").join("opencode.json")).unwrap();
    assert!(cloned.contains("dark") && cloned.contains("example/model"));
    assert!(!cloned.contains("secret") && !cloned.contains("provider"));
    assert!(!config(home, "arman").join("auth.json").exists());
    assert!(!data(home, "arman").join("credential.txt").exists());
    assert!(!invoke(home, &["remove", "arman"]).status.success());
    assert!(invoke(home, &["remove", "arman", "--yes"]).status.success());
    assert!(!config(home, "arman").exists());
    assert!(config(home, "hp").exists());
}

#[test]
fn legacy_roots_are_not_discovered_or_modified() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let legacy = home.join("config/opencode-multi/profiles/hp");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("auth.json"), "legacy credential sentinel").unwrap();
    let output = invoke(home, &["list"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(invoke(home, &["create", "hp"]).status.success());
    assert!(config(home, "hp").join("opencode.json").exists());
    assert_eq!(
        fs::read_to_string(legacy.join("auth.json")).unwrap(),
        "legacy credential sentinel"
    );
}

#[cfg(unix)]
#[test]
fn run_isolates_profiles_and_forwards_arguments_and_exit_code() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    for name in ["hp", "arman"] {
        assert!(invoke(home, &["create", name]).status.success());
    }
    let path = install_fake_opencode(home);
    let routes: &[(&[&str], &str)] = &[
        (&[], "ARG=\n"),
        (&["auth", "list"], "ARG=auth\nARG=list\n"),
        (&["debug", "paths", "db"], "ARG=debug\nARG=paths\nARG=db\n"),
        (
            &["run", "--model", "example/model", "hello world"],
            "ARG=run\nARG=--model\nARG=example/model\nARG=hello world\n",
        ),
    ];
    let mut outputs = Vec::new();
    for name in ["hp", "arman"] {
        for (route, expected) in routes {
            let output = Command::new(env!("CARGO_BIN_EXE_opencode-profile"))
                .args(["run", name, "--"])
                .args(*route)
                .env("PATH", &path)
                .env("HOME", home)
                .env("XDG_CONFIG_HOME", home.join("config"))
                .env("XDG_DATA_HOME", home.join("data"))
                .env("XDG_STATE_HOME", home.join("state"))
                .env("XDG_CACHE_HOME", home.join("cache"))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(23));
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(&format!("CONFIG={}\n", config(home, name).display())));
            assert!(text.contains(&format!("DATA={}\n", data(home, name).display())));
            assert!(text.contains(&format!(
                "STATE={}\n",
                home.join("state/opencode-profile/profiles")
                    .join(name)
                    .display()
            )));
            assert!(text.contains(&format!(
                "CACHE={}\n",
                home.join("cache/opencode-profile/profiles")
                    .join(name)
                    .display()
            )));
            assert!(text.contains(&format!(
                "XDG_CONFIG={}\n",
                config(home, name).join(".xdg").display()
            )));
            assert!(text.ends_with(expected), "{text}");
            if *route == ["debug", "paths", "db"] {
                outputs.push(text);
            }
        }
    }
    assert_ne!(outputs[0], outputs[1]);
    for args in [
        vec!["run", "hp", "--", "unknown-command"],
        vec!["run", "hp", "--", "--log-level", "info", "unknown-command"],
        vec!["run", "hp", "--", "auth", "list", "--server=localhost"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_opencode-profile"))
            .args(&args)
            .env("PATH", &path)
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }
    assert!(!data(home, "hp").join("opencode").exists()); // Fake executable never writes credentials.
}

#[test]
fn create_does_not_import_other_opencode_configs() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    fs::create_dir_all(home.join("config/opencode")).unwrap();
    fs::write(
        home.join("config/opencode/opencode.json"),
        r#"{"theme":"light","provider":{"token":"secret"}}"#,
    )
    .unwrap();
    assert!(invoke(home, &["create", "test"]).status.success());
    let content = fs::read_to_string(config(home, "test").join("opencode.json")).unwrap();
    assert!(!content.contains("light"));
    assert!(!content.contains("secret"));
}

#[cfg(unix)]
#[test]
fn persistent_services_reuse_are_separate_and_have_lifecycle_controls() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    for name in ["hp", "arman"] {
        assert!(invoke(home, &["create", name]).status.success());
        assert!(fake_command(home, &path, &["service", name, "status"])
            .output()
            .unwrap()
            .status
            .success());
        assert!(!state(home, name).join("fake-calls").exists());
        for _ in 0..2 {
            let output = fake_command(home, &path, &["run", name, "--", "auth", "list"])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(23), "{:?}", output.stderr);
        }
        assert_eq!(
            fs::read_to_string(state(home, name).join("fake-starts"))
                .unwrap()
                .trim(),
            "1"
        );
        assert!(fake_command(home, &path, &["service", name, "start"])
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(
            fs::read_to_string(state(home, name).join("fake-starts"))
                .unwrap()
                .trim(),
            "1"
        );
    }
    let read_registration = |name| {
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(state(home, name).join("opencode/service.json")).unwrap(),
        )
        .unwrap()
    };
    assert_ne!(
        read_registration("hp")["url"],
        read_registration("arman")["url"]
    );
    assert!(fake_command(home, &path, &["service", "hp", "restart"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(
        fs::read_to_string(state(home, "hp").join("fake-starts"))
            .unwrap()
            .trim(),
        "2"
    );
    for _ in 0..2 {
        assert!(fake_command(home, &path, &["service", "hp", "stop"])
            .output()
            .unwrap()
            .status
            .success());
    }
    assert!(!state(home, "hp").join("opencode/service.json").exists());
    assert!(state(home, "arman").join("opencode/service.json").exists());
}

#[cfg(unix)]
#[test]
fn concurrent_clients_start_one_service_and_removal_stops_it() {
    use std::process::Stdio;
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    let children: Vec<_> = (0..4)
        .map(|_| {
            fake_command(home, &path, &["run", "hp", "--", "models"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(23),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read_to_string(state(home, "hp").join("fake-starts"))
            .unwrap()
            .trim(),
        "1"
    );
    fs::write(state(home, "hp").join("fake-stop-fail"), "").unwrap();
    assert!(!fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(config(home, "hp").exists());
    assert!(state(home, "hp").join("opencode/service.json").exists());
    fs::remove_file(state(home, "hp").join("fake-stop-fail")).unwrap();
    assert!(fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(!config(home, "hp").exists());
    assert!(!state(home, "hp").exists());
}

#[cfg(unix)]
#[test]
fn local_and_rejected_routes_do_not_start_services() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    for args in [
        vec!["run", "hp", "--", "debug", "paths", "db"],
        vec!["run", "hp", "--", "--help"],
        vec!["run", "hp", "--", "--version"],
        vec![
            "run",
            "hp",
            "--",
            "auth",
            "list",
            "--server=http://example.invalid",
        ],
        vec!["run", "hp", "--", "--standalone=true"],
        vec!["run", "hp", "--", "--no-standalone=false"],
        vec!["run", "hp", "--", "unknown-command"],
    ] {
        let output = fake_command(home, &path, &args).output().unwrap();
        if args.iter().any(|arg| {
            arg.starts_with("--server=")
                || arg.starts_with("--standalone=")
                || arg.starts_with("--no-standalone=")
                || *arg == "unknown-command"
        }) {
            assert!(!output.status.success(), "{args:?}");
        }
        assert!(!state(home, "hp").join("fake-calls").exists(), "{args:?}");
    }
}

#[cfg(unix)]
#[test]
fn startup_failure_prevents_client_launch() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    fs::create_dir_all(state(home, "hp")).unwrap();
    fs::write(state(home, "hp").join("fake-start-fail"), "").unwrap();
    let output = fake_command(home, &path, &["run", "hp"]).output().unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("CONFIG="));
    assert!(!state(home, "hp").join("opencode/service.json").exists());
}

#[cfg(unix)]
#[test]
fn changed_roots_and_foreign_registration_are_rejected() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    assert!(fake_command(home, &path, &["service", "hp", "start"])
        .output()
        .unwrap()
        .status
        .success());
    let before = fs::read(state(home, "hp").join("fake-calls")).unwrap();
    for key in ["XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"] {
        let output = fake_command(home, &path, &["run", "hp"])
            .env(key, home.join(format!("other-{key}")))
            .output()
            .unwrap();
        assert!(!output.status.success(), "{key}");
        assert_eq!(
            fs::read(state(home, "hp").join("fake-calls")).unwrap(),
            before
        );
    }
    let record = state(home, "hp").join("opencode/service.json");
    let mut foreign: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    foreign["url"] = serde_json::json!("http://127.0.0.1:1");
    fs::write(&record, serde_json::to_vec(&foreign).unwrap()).unwrap();
    for args in [
        vec!["run", "hp"],
        vec!["service", "hp", "stop"],
        vec!["remove", "hp", "--yes"],
    ] {
        assert!(
            !fake_command(home, &path, &args)
                .output()
                .unwrap()
                .status
                .success(),
            "{args:?}"
        );
    }
    assert_eq!(
        fs::read(state(home, "hp").join("fake-calls")).unwrap(),
        before
    );
    assert!(config(home, "hp").exists());
}

#[cfg(unix)]
#[test]
fn unmanaged_and_symlinked_service_registration_are_rejected() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    let record = state(home, "hp").join("opencode/service.json");
    fs::create_dir_all(record.parent().unwrap()).unwrap();
    fs::write(&record, "{}").unwrap();
    assert!(!fake_command(home, &path, &["service", "hp", "start"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(!state(home, "hp").join("fake-calls").exists());
    fs::remove_file(&record).unwrap();
    let outside = home.join("outside-service.json");
    fs::write(&outside, "sentinel").unwrap();
    symlink(&outside, &record).unwrap();
    assert!(!fake_command(home, &path, &["service", "hp", "stop"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read_to_string(outside).unwrap(), "sentinel");
    assert!(!state(home, "hp").join("fake-calls").exists());
}

#[cfg(unix)]
#[test]
fn delayed_client_cannot_resurrect_a_removed_service() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::Duration;
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    fs::write(home.join("pause-client"), "").unwrap();
    let mut child = fake_command(home, &path, &["run", "hp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, recv) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line);
        let _ = send.send((result, line));
        std::io::copy(&mut reader, &mut std::io::sink()).unwrap();
    });
    let ready = recv.recv_timeout(Duration::from_secs(10));
    if ready.is_err() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("Client failed to reach deterministic startup barrier");
    }
    assert_eq!(ready.unwrap().1, "CLIENT_READY\n");
    let removal = fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    assert!(
        removal.status.success(),
        "{}",
        String::from_utf8_lossy(&removal.stderr)
    );
    assert!(!output.status.success());
    assert!(!config(home, "hp").exists());
    assert!(!data(home, "hp").exists());
    assert!(!state(home, "hp").exists());
}

#[cfg(unix)]
#[test]
fn help_shaped_prompt_does_not_bypass_service_identity_checks() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    assert!(fake_command(home, &path, &["service", "hp", "start"])
        .output()
        .unwrap()
        .status
        .success());
    fs::write(
        state(home, "hp").join("opencode-profile-service.json"),
        "{}",
    )
    .unwrap();
    let before = fs::read(state(home, "hp").join("fake-calls")).unwrap();
    let output = fake_command(home, &path, &["run", "hp", "--", "--prompt", "--help"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        fs::read(state(home, "hp").join("fake-calls")).unwrap(),
        before
    );
}

#[cfg(target_os = "linux")]
#[test]
fn autostart_generates_private_isolated_units_without_credentials() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    for _ in 0..2 {
        let output = fake_command(home, &path, &["service", "hp", "autostart", "enable"])
            .env("OPENAI_API_KEY", "sensitive-test-sentinel")
            .env("OPENCODE_SERVER_PASSWORD", "password-test-sentinel")
            .env("OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS", "true")
            .env("OPENCODE_ENABLE_EXA", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let files = unit_files(home, "hp");
    assert_eq!(files.len(), 1);
    assert_eq!(
        fs::metadata(&files[0]).unwrap().permissions().mode() & 0o077,
        0
    );
    let unit = fs::read_to_string(&files[0]).unwrap();
    for expected in [
        "[Unit]",
        "[Service]",
        "[Install]",
        "Type=oneshot",
        "RemainAfterExit=yes",
        "WantedBy=default.target",
        "ExecStart=",
        "ExecStop=",
        "service",
        "hp",
        "start",
        "stop",
        "HOME=",
        "PATH=",
        "XDG_CONFIG_HOME=",
        "XDG_DATA_HOME=",
        "XDG_STATE_HOME=",
        "XDG_CACHE_HOME=",
    ] {
        assert!(unit.contains(expected), "Missing {expected}: {unit}");
    }
    for root in [
        home.to_path_buf(),
        home.join("config"),
        home.join("data"),
        home.join("state"),
        home.join("cache"),
    ] {
        assert!(
            unit.contains(root.to_str().unwrap()),
            "Missing root {}",
            root.display()
        );
    }
    assert!(!unit.contains("sensitive-test-sentinel"));
    assert!(!unit.contains("password-test-sentinel"));
    assert!(!unit.contains("OPENAI_API_KEY"));
    let calls = fs::read_to_string(home.join("fake-systemctl-calls")).unwrap();
    assert!(calls.contains("enable") && calls.contains("--now"));
    assert!(calls.contains(files[0].to_str().unwrap()));
    assert!(!calls.contains("--system"));
    assert!(!calls.contains("linger"));
}

#[cfg(target_os = "linux")]
#[test]
fn services_lists_runtime_and_autostart_without_starting_profiles() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    for name in ["hp", "arman"] {
        assert!(invoke(home, &["create", name]).status.success());
    }
    let stopped = fake_command(home, &path, &["services"]).output().unwrap();
    assert!(stopped.status.success());
    let text = String::from_utf8(stopped.stdout).unwrap();
    assert!(
        text.contains("Profile") && text.contains("Service") && text.contains("Autostart"),
        "{text}"
    );
    assert!(
        text.contains("hp")
            && text.contains("arman")
            && text.contains("stopped")
            && text.contains("disabled"),
        "{text}"
    );
    for name in ["hp", "arman"] {
        assert!(!state(home, name).join("fake-calls").exists());
    }
    assert!(fake_command(home, &path, &["service", "hp", "start"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(
        fake_command(home, &path, &["service", "hp", "autostart", "enable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let live = fake_command(home, &path, &["services"]).output().unwrap();
    assert!(live.status.success());
    let text = String::from_utf8(live.stdout).unwrap();
    let hp = text.lines().find(|line| line.contains("hp")).unwrap();
    let arman = text.lines().find(|line| line.contains("arman")).unwrap();
    assert!(hp.contains("running") && hp.contains("enabled"), "{text}");
    assert!(
        arman.contains("stopped") && arman.contains("disabled"),
        "{text}"
    );
    assert!(!text.contains('\x1b'));
    assert!(!text.contains("password"));
    assert_eq!(
        fs::read_to_string(state(home, "hp").join("fake-starts"))
            .unwrap()
            .trim(),
        "1"
    );
    fs::write(state(home, "hp").join("fake-stale"), "").unwrap();
    let stale = fake_command(home, &path, &["services"]).output().unwrap();
    let text = String::from_utf8(stale.stdout).unwrap();
    let hp = text.lines().find(|line| line.contains("hp")).unwrap();
    assert!(
        !hp.contains("running"),
        "Stale registration must not be called running: {text}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn disabling_autostart_keeps_service_running_and_removal_cleans_both() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    fs::write(home.join("simulate-unit-callback"), "").unwrap();
    assert!(invoke(home, &["create", "hp"]).status.success());
    let enabled = fake_command(home, &path, &["service", "hp", "autostart", "enable"])
        .output()
        .unwrap();
    assert!(
        enabled.status.success(),
        "{}",
        String::from_utf8_lossy(&enabled.stderr)
    );
    assert!(state(home, "hp").join("opencode/service.json").exists());
    let disabled = fake_command(home, &path, &["service", "hp", "autostart", "disable"])
        .output()
        .unwrap();
    assert!(
        disabled.status.success(),
        "{}",
        String::from_utf8_lossy(&disabled.stderr)
    );
    assert!(state(home, "hp").join("opencode/service.json").exists());
    assert!(
        fake_command(home, &path, &["service", "hp", "autostart", "enable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let unit = unit_files(home, "hp").pop().unwrap();
    let removed = fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!unit.exists());
    assert!(!config(home, "hp").exists());
    assert!(!state(home, "hp").exists());
    let calls = fs::read_to_string(home.join("fake-systemctl-calls")).unwrap();
    assert!(
        calls.lines().any(|line| line.starts_with("disable")),
        "{calls}"
    );
    assert!(
        calls.lines().any(|line| line.starts_with("stop")
            || (line.starts_with("disable") && line.contains("--now"))),
        "{calls}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn autostart_failures_are_not_reported_as_enabled_and_preserve_profiles() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    fs::write(home.join("fail-enable"), "").unwrap();
    let output = fake_command(home, &path, &["service", "hp", "autostart", "enable"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let listed = fake_command(home, &path, &["services"]).output().unwrap();
    let text = String::from_utf8(listed.stdout).unwrap();
    assert!(
        !text
            .lines()
            .any(|line| line.contains("hp") && line.contains("enabled")),
        "{text}"
    );
    fs::remove_file(home.join("fail-enable")).unwrap();
    assert!(
        fake_command(home, &path, &["service", "hp", "autostart", "enable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::write(home.join("fail-disable"), "").unwrap();
    assert!(!fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(config(home, "hp").exists());
    assert_eq!(unit_files(home, "hp").len(), 1);
    fs::remove_file(home.join("fail-disable")).unwrap();
    fs::write(home.join("manager-unavailable"), "").unwrap();
    let listed = fake_command(home, &path, &["services"]).output().unwrap();
    assert!(listed.status.success());
    let text = String::from_utf8(listed.stdout).unwrap();
    assert!(text.contains("unavailable"), "{text}");
    assert!(!fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(config(home, "hp").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn autostart_rejects_symlink_directories_and_missing_profiles() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    assert!(
        !fake_command(home, &path, &["service", "missing", "autostart", "enable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(invoke(home, &["create", "hp"]).status.success());
    let outside = home.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, config(home, "hp").join("autostart")).unwrap();
    assert!(
        !fake_command(home, &path, &["service", "hp", "autostart", "enable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(fs::read_dir(outside).unwrap().next().is_none());
    assert!(!home.join("fake-systemctl-calls").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn autostart_unit_names_include_effective_roots_and_reenable_refreshes_flags() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    let args = ["service", "hp", "autostart", "enable"];
    assert!(fake_command(home, &path, &args)
        .env("OPENCODE_ENABLE_EXA", "0")
        .output()
        .unwrap()
        .status
        .success());
    let original = unit_files(home, "hp").pop().unwrap();
    assert!(fs::read_to_string(&original)
        .unwrap()
        .contains("OPENCODE_ENABLE_EXA=0"));
    assert!(fake_command(home, &path, &args)
        .env("OPENCODE_ENABLE_EXA", "1")
        .output()
        .unwrap()
        .status
        .success());
    assert!(fs::read_to_string(&original)
        .unwrap()
        .contains("OPENCODE_ENABLE_EXA=1"));
    for args in [
        vec!["create", "hp"],
        vec!["service", "hp", "autostart", "enable"],
    ] {
        let output = fake_command(home, &path, &args)
            .env("XDG_CONFIG_HOME", home.join("other-config"))
            .env("XDG_DATA_HOME", home.join("other-data"))
            .env("XDG_STATE_HOME", home.join("other-state"))
            .env("XDG_CACHE_HOME", home.join("other-cache"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let other_dir = home.join("other-config/opencode-profile/profiles/hp/autostart");
    let other = fs::read_dir(other_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "service"))
        .unwrap();
    assert_ne!(original.file_name(), other.file_name());
}

#[cfg(target_os = "linux")]
#[test]
fn autostart_escapes_paths_and_rejects_newlines() {
    let dir = TempDir::new().unwrap();
    let special_home = dir.path().join("home % $ \" path");
    fs::create_dir(&special_home).unwrap();
    let home = special_home.as_path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    let output = fake_command(home, &path, &["service", "hp", "autostart", "enable"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let file = unit_files(home, "hp").pop().unwrap();
    let contents = fs::read_to_string(&file).unwrap();
    assert!(contents.contains("home %% $ \\\" path"), "{contents}");
    let before = fs::read(&file).unwrap();
    let rejected = fake_command(home, &path, &["service", "hp", "autostart", "enable"])
        .env("PATH", format!("{path}\nmalformed-unit-injection"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(fs::read(file).unwrap(), before);
}

#[cfg(target_os = "linux")]
#[test]
fn removing_disabled_inactive_autostart_and_incomplete_profiles_works() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = install_fake_opencode(home);
    install_fake_systemctl(home);
    assert!(invoke(home, &["create", "hp"]).status.success());
    assert!(
        fake_command(home, &path, &["service", "hp", "autostart", "enable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        fake_command(home, &path, &["service", "hp", "autostart", "disable"])
            .output()
            .unwrap()
            .status
            .success()
    );
    for entry in fs::read_dir(home.join("fake-systemd")).unwrap() {
        let file = entry.unwrap().path();
        if file.extension().is_some_and(|ext| ext == "active") {
            fs::remove_file(file).unwrap();
        }
    }
    let removal = fake_command(home, &path, &["remove", "hp", "--yes"])
        .output()
        .unwrap();
    assert!(
        removal.status.success(),
        "{}",
        String::from_utf8_lossy(&removal.stderr)
    );
    fs::create_dir_all(data(home, "partial")).unwrap();
    assert!(fake_command(home, &path, &["remove", "partial", "--yes"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(!data(home, "partial").exists());
}
