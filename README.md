# opencode-profile

Rust CLI for isolated OpenCode profiles. It uses its own roots and never reads `opencode-multi` profiles.

## Installation

All installation methods build from source and require a working [Rust/Cargo toolchain](https://rustup.rs/) and the platform's native build tools. Git-based installation also requires Git. The script is a Cargo wrapper, not a prebuilt binary downloader.

Source: [pouralijan/opencode-profile](https://github.com/pouralijan/opencode-profile). The default branch is `dev`.

### One-line script (Bash)

```sh
curl -fsSL https://raw.githubusercontent.com/pouralijan/opencode-profile/dev/install.sh | bash -s -- https://github.com/pouralijan/opencode-profile.git
```

This requires `curl` and Bash in addition to Cargo and Git. Review the script before executing downloaded code. To download and inspect it first:

```sh
curl -fsSL https://raw.githubusercontent.com/pouralijan/opencode-profile/dev/install.sh -o install.sh
less install.sh
bash install.sh https://github.com/pouralijan/opencode-profile.git
```

### Cargo

Install directly from the repository:

```sh
cargo install --locked --git https://github.com/pouralijan/opencode-profile.git opencode-profile
```

Or, from a local checkout:

```sh
cargo install --locked --path .
```

Cargo installs binaries into `$CARGO_INSTALL_ROOT/bin`, or `$CARGO_HOME/bin` (default `~/.cargo/bin`). Ensure that directory is in `PATH`. For the default location on Bash/Zsh:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
opencode-profile --version
opencode-profile --help
```

Rerun the same installation command to update after source changes. To uninstall:

```sh
cargo uninstall opencode-profile
```

On Linux, disable any enabled autostart with `opencode-profile service NAME autostart disable`, then stop each running profile with `opencode-profile service NAME stop` before uninstalling. Removing the launcher does not stop services or disable startup units. Uninstalling leaves profile directories untouched. Use `opencode-profile remove NAME` beforehand only if you also want to delete that profile's config, data, state, and cache.

## Quick start

`run` and service management need an `opencode` executable in `PATH` with native `service` commands and authenticated `--server` attachment (verified with OpenCode v2.1.3). `exec` needs the program you invoke (for example, Bun for `bunx`). Creating, listing, showing, and cloning profiles do not require OpenCode. Removing a running profile requires OpenCode to stop its service safely.

```sh
opencode-profile create hp
opencode-profile list
opencode-profile services
opencode-profile show hp
opencode-profile run hp -- auth login
opencode-profile run hp
```

## CLI usage

```text
opencode-profile <COMMAND>
```

| Command | Usage | Purpose |
| --- | --- | --- |
| `create` | `create NAME` | Create a fresh, independent profile |
| `list` | `list` | List profiles and directory readiness |
| `show` | `show NAME` | Show profile paths and readiness |
| `run` | `run NAME [-- OPENCODE_ARGS...]` | Start or reuse the profile's persistent service |
| `service` | `service NAME start\|status\|stop\|restart` | Manage a profile's service |
| `service … autostart` | `service NAME autostart enable\|disable\|status` | Manage Linux systemd user autostart |
| `services` | `services` | List runtime health and autostart status for all known profiles |
| `exec` | `exec NAME "COMMAND"` or `exec NAME -- PROGRAM ARG...` | Run a command in a profile environment |
| `clone` | `clone SOURCE DESTINATION` | Copy safe display/model preferences into a new profile |
| `remove` | `remove NAME [--yes]` | Delete all four profile directories |
| `doctor` | `doctor` | Show OpenCode's location, roots, and profile readiness |

```sh
opencode-profile --help
opencode-profile --version
opencode-profile help run
opencode-profile help exec
opencode-profile service hp --help
opencode-profile remove --help
```

Use `help COMMAND` to inspect launcher help unambiguously. Use `--` after the profile name when forwarding options to another program.

Names must contain 1–64 ASCII letters, digits, hyphens, or underscores and start with a letter or digit. Paths are resolved from the invoking environment: `HOME` must be set and XDG homes must resolve to absolute paths.

`create` always starts fresh and reads no existing configuration. `clone` copies only `$schema`, `theme`, `model`, and `small_model` string preferences from another `opencode-profile` profile. It does not copy plugins, commands, agents, other config keys, auth, data, or autostart units. Review and transfer any extra configuration yourself. `remove` disables and cleans up any owned autostart unit, then stops the profile's managed service before deleting config, data, state, and cache; failed cleanup, shutdown, or ownership checks preserve the profile directories. Without `--yes`, removal requires an interactive terminal and typing the profile name.

### Run OpenCode

```sh
opencode-profile run hp
opencode-profile run hp -- auth list
opencode-profile run hp -- models --refresh
opencode-profile run hp -- run --model example/model "hello world"
opencode-profile run hp -- debug paths db
```

Supported routes are the interactive root command; `run`, `mini`, `models`, `stats`, `api`, `reload`; `auth list|login|logout|export|import|switch`; `session list|delete|export|import`; and `debug paths`. Root options are restricted to the launcher's allowlist; unknown routes fail closed. The installed OpenCode version must support the selected route and its `--server` flag. `debug paths` and single-flag root help/version calls remain local and do not start services.

### Persistent services

The first `run` starts a native OpenCode service bound to `127.0.0.1` on a port selected for that profile. Later clients attach to the same authenticated service; closing a client does **not** stop it. Other profiles have separate service registrations and ports, and multiple clients can use one profile. This avoids repeating service startup, but does not eliminate provider/network latency or all per-project initialization.

```sh
opencode-profile service hp start     # Optional: run starts it automatically
opencode-profile service hp status    # Does not start a stopped service
opencode-profile service hp restart
opencode-profile service hp stop
```

`stop` is a success when already stopped. `restart` and `stop` affect all clients using that profile. After a stop, a new `run` starts the service again. Lifecycle operations and removal are serialized by a profile-state lock; clients attach to a captured endpoint without discovery fallback, so a client racing with removal fails rather than resurrecting the service.

The service keeps the environment from its startup. To change feature flags, inherited provider tokens, or other startup settings, restart it with the desired environment:

```sh
OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS=true OPENCODE_ENABLE_EXA=1 \
  opencode-profile service hp restart
opencode-profile run hp
```

Use `run NAME -- reload` for OpenCode's configuration reload where supported; it is not an environment reload. Ports are selected by briefly reserving an available loopback port before native startup. If another process claims it in between, startup fails closed rather than attaching to that process; the error explains safe port reallocation.

The launcher owns `service.json` in the profile config directory (`OPENCODE_CONFIG_DIR`); OpenCode adds its generated service password there. Root ownership is recorded in `opencode-profile-service.json` in both profile config and state directories. Native registration, also containing the authentication secret, stays at `opencode/service.json` in the profile state directory. Do not copy these files between profiles or share them. Launcher-created JSON metadata is owner-only on Unix. State-namespace lock files live outside removable profile directories at `$XDG_STATE_HOME/opencode-profile/service-locks/NAME.lock`.

Unmanaged native service metadata, altered settings, symlinked runtime metadata, and mismatched roots are refused rather than adopted. Keep all four XDG roots consistent across invocations. Existing profiles created by the old standalone launcher need no migration if they have no native service metadata. Never delete a live registration to bypass an ownership error; stop it through its original environment first.

### Autostart after login or reboot (Linux)

Install the launcher to a stable location first, such as with `cargo install --locked --path .`. Autostart requires a working **systemd user manager**, and never uses `sudo` or changes account settings automatically.

```sh
opencode-profile service hp autostart enable   # Enable and start now
opencode-profile service arman autostart enable
opencode-profile services
opencode-profile service hp autostart status
opencode-profile service hp autostart disable  # Leave the running service alone
```

By default, user units start when your user manager starts, usually **at login**. To start them at boot without logging in, optionally enable lingering for your account:

```sh
loginctl enable-linger "$USER"
```

This is a separate account-level choice and may require administrator authorization. Boot startup also requires the launcher and profile storage to be accessible before login. No autostart is installed merely by creating or running a profile.

Each profile gets a private unit under its config directory at `autostart/opencode-profile-NAME-HASH.service`; the hash distinguishes the four effective XDG roots. The unit captures the launcher's absolute executable path, HOME, PATH, and the four base XDG homes. **API keys, provider tokens, service passwords, and arbitrary environment variables are not written into units.** Authenticate inside each profile normally.

The non-secret boolean flags `OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS` and `OPENCODE_ENABLE_EXA` can be captured explicitly when enabling:

```sh
OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS=true OPENCODE_ENABLE_EXA=1 \
  opencode-profile service hp autostart enable
```

Re-enable to refresh the executable path, PATH, or those flags after changes; restart the native service to apply a changed startup environment. Modified/unowned unit files are rejected rather than overwritten. Removal stops an active unit and removes its enablement before deleting the profile; if the manager is unavailable, cleanup fails safely instead of leaving an enabled unit pointing to deleted data.

Autostart is a systemd oneshot start/stop action, **not continuous crash supervision**. A unit being enabled does not prove its native service is running. If you manually stop the service, use `service NAME start` to bring it back immediately.

### List all profile services

```sh
opencode-profile services
```

The table lists profiles in the current XDG roots, with independent **Service** (`running`, `stopped`, `unhealthy`, or `unknown`) and **Autostart** (`enabled`, `disabled`, or `unavailable`) columns. Runtime health is queried rather than inferred from registration-file existence. Listing never starts services or enables units, continues past unhealthy profiles, and never displays service credentials. Without a working systemd user manager, autostart is reported as unavailable.

### Output

`list`, `show`, `doctor`, and `services` display rounded tables with Unicode icons for fields and status. Success messages use `✓`; warnings use `⚠`. An empty `list` produces no output; empty `services` reports no profiles found. Output from `run` is forwarded unchanged.

In terminals, icons are green for success, yellow for warnings, and cyan for information; error labels are red. Redirected output, `TERM=dumb`, and a non-empty `NO_COLOR` disable color. These settings affect only the launcher, not forwarded OpenCode output.

### Run other commands in a profile

`exec NAME "COMMAND"` runs one command string through `sh -c` on Unix (`cmd.exe /C` elsewhere). With multiple arguments, `exec NAME -- PROGRAM ARG...` executes the program directly, preserving argument boundaries. Both forms require an existing profile, use the same profile environment and path checks as `run`, inherit the current working directory and standard streams, and return the child exit status.

A single program name also goes through the shell, even after `--`. Shell strings use the system shell, not your interactive shell's aliases or functions.

Use single quotes around shell strings when variables should expand inside the profile environment:

```sh
opencode-profile exec hp 'printf "%s\n" "$OPENCODE_CONFIG_DIR"'
opencode-profile exec hp -- bunx oh-my-opencode-slim@latest install --no-tui --background-subagents=no
OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS=true OPENCODE_ENABLE_EXA=1 opencode-profile run hp
```

`exec` is an explicit escape hatch, **not a sandbox**: it does not validate commands, ensure a managed service, or enforce service attachment. Prefer `run` for OpenCode itself. Commands can still access your home, network, inherited credentials, and shell startup files; installers must honor the profile environment. For oh-my-opencode-slim, `--background-subagents=no` avoids editing shared shell startup files; supply the required flags on the service's first launch or restart it with those flags.

## Profile paths and isolation

Profiles use `$XDG_CONFIG_HOME/opencode-profile/profiles/NAME` and `$XDG_DATA_HOME/opencode-profile/profiles/NAME`. State and cache use matching roots under `$XDG_STATE_HOME` and `$XDG_CACHE_HOME`. Defaults are `~/.config`, `~/.local/share`, `~/.local/state`, and `~/.cache`. Existing `opencode-multi` profiles are **not** detected, imported, or accessed automatically.

`run`, service management, and `exec` set these child environment variables:

| Variable | Value |
| --- | --- |
| `OPENCODE_PROFILE` | Profile name |
| `OPENCODE_CONFIG_DIR` | Profile config directory |
| `XDG_CONFIG_HOME` | Profile config directory + `/.xdg` |
| `XDG_DATA_HOME` | Profile data directory |
| `XDG_STATE_HOME` | Profile state directory |
| `XDG_CACHE_HOME` | Profile cache directory |

Each run sets `OPENCODE_CONFIG_DIR` to the new profile config directory. It sets `XDG_CONFIG_HOME` to `.../profiles/NAME/.xdg` so OpenCode CLI preferences live at `.xdg/opencode/cli.json`, not in the shared `$XDG_CONFIG_HOME/opencode/cli.json`. Existing global or profile-root `cli.json` is not imported. It also sets per-profile `XDG_STATE_HOME` and `XDG_CACHE_HOME` under `opencode-profile/profiles/NAME`. Run `opencode-profile run NAME -- auth list` to inspect authentication; the launcher never claims a profile is authenticated.

For managed routes, the launcher inserts its verified `--server URL` at the route's supported position and supplies the captured service password through the child's environment, never command-line arguments. Caller-selected `--server`, `--standalone`, and `--no-standalone` flags (including `=...` forms) are rejected. Startup and ownership failures abort the command with no global-service fallback. Remaining arguments and child exit status are preserved. OpenCode may still use global resources or external account services; profile isolation is not a filesystem, network, or environment sandbox.

Profile status means directories exist, **not** authenticated: OpenCode versions may store auth in `auth.json` or a database. Check authentication with OpenCode itself from the target profile. Project-local OpenCode config and inherited environment variables can override profile settings.

## Optional manual migration

For optional **manual config-only migration**, run `opencode-profile create NAME`, inspect your old `~/.config/opencode-multi/profiles/NAME/opencode.json`, then manually enter non-secret settings into `~/.config/opencode-profile/profiles/NAME/opencode.json`. Review each setting before transferring it; provider and plugin settings may embed tokens. Do **not** copy entire directories, credentials, `auth.json`, `cli.json`, databases, data, state, or cache. Authenticate separately within each new profile. `opencode-multi` 0.2.1 could store credentials under a shared data path; never copy those credentials into a new profile.

## Development and verification

### Code organization

- `src/main.rs`: parse arguments, report errors, and exit.
- `src/cli.rs`: command definitions and help text.
- `src/app.rs`: command dispatch and interactive removal confirmation.
- `src/profile.rs`: profile roots, validation, lifecycle, and safe clone settings.
- `src/process.rs`: OpenCode route validation, profile environment, and child execution.
- `src/service.rs`: service ownership, per-profile lifecycle locking, and authenticated client attachment.
- `src/autostart.rs`: owned systemd user units, safe environment capture, and autostart cleanup.
- `src/output.rs`: tables, icons, and terminal colors.

```sh
cargo fmt --check
cargo test
cargo check
# Focus on exec integration tests:
cargo test --test cli exec_
# Focus on persistent-service lifecycle tests:
cargo test --test cli service
# Focus on mocked systemd/autostart checks:
cargo test --test cli autostart_
```

Integration tests use temporary homes, fake OpenCode/systemctl executables, and local shell scripts; no provider account, real credentials, or live systemd user manager is needed. They cover help, tables, profile lifecycle, service reuse/separation, concurrent clients, safe removal, ownership failures, explicit attachment, autostart generation/cleanup/failures, and shell/direct `exec` behavior. Shell execution, fake services, and symlink tests are Unix-only; systemd tests are Linux-only.
