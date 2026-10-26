use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    version,
    about = "Manage isolated OpenCode profiles and run commands in their environment",
    long_about = "Manage independent OpenCode config, data, state, and cache directories.\nUse run to reuse one persistent native service per profile; use service\nto manage its lifecycle and exec as an unmanaged escape hatch.\nNeither command is a filesystem or network sandbox.",
    after_help = "Examples:\n  opencode-profile create hp\n  opencode-profile run hp\n  opencode-profile run hp -- auth login\n  opencode-profile exec hp \"bunx oh-my-opencode-slim@latest install --no-tui --background-subagents=no\"\n\nUse 'opencode-profile <COMMAND> --help' for command details."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Action,
}

#[derive(Subcommand)]
pub(crate) enum Action {
    /// Create a fresh profile without importing config or credentials.
    #[command(
        after_help = "Example: opencode-profile create hp\nNames: 1-64 ASCII letters, digits, hyphens or underscores; start with a letter or digit."
    )]
    Create {
        /// New profile name.
        name: String,
    },
    /// List profiles and directory readiness (authentication is not checked).
    List,
    /// List runtime service health and reboot autostart independently.
    Services,
    /// Show a profile's paths and directory readiness.
    Show {
        /// Existing profile name.
        name: String,
    },
    /// Launch OpenCode using a profile's persistent native service.
    #[command(
        long_about = "Launch OpenCode with profile-specific config, data, state, and cache.\nSupported routes attach explicitly to the verified persistent profile service\nURL; the launcher supplies endpoint and authentication, without discovery\nor fallback. Debug paths and a single root help/version flag remain local.\nUnknown routes and caller-supplied server/standalone flags are rejected.\nRestart the service to apply changes to its startup environment.",
        after_help = "Examples:\n  opencode-profile run hp\n  opencode-profile run hp -- auth login\n  opencode-profile run hp -- auth list\n  opencode-profile run hp -- run --model example/model \"hello world\"\n  opencode-profile run hp -- debug paths db\n\nUse '--' after the profile name to forward OpenCode options, including --help."
    )]
    Run {
        /// Existing profile name.
        name: String,
        /// OpenCode arguments; omit to launch the interactive TUI.
        #[arg(
            value_name = "OPENCODE_ARGS",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    /// Manage the profile's persistent native OpenCode service.
    #[command(
        after_help = "The service survives client exit. Restart to apply startup environment changes.\nExample: opencode-profile service hp restart"
    )]
    Service {
        name: String,
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Execute a shell command or program with this profile's environment (not a sandbox).
    #[command(
        long_about = "Execute a command with the same profile environment and path checks as run.\nOne argument runs through sh -c on Unix (cmd.exe /C elsewhere). Multiple\narguments execute the program directly. Inherits the working directory and\nstandard streams, and returns the child exit status.\n\nThis is an escape hatch: no OpenCode route validation or managed service\nlifecycle. Programs may access shared services, home files, and credentials.\nUse run for OpenCode itself.",
        after_help = "Examples:\n  opencode-profile exec hp \"bunx oh-my-opencode-slim@latest install --no-tui --background-subagents=no\"\n  opencode-profile exec hp -- bunx oh-my-opencode-slim@latest install --no-tui --background-subagents=no\n  opencode-profile exec hp 'printf \"%s\\n\" \"$OPENCODE_CONFIG_DIR\"'\n\nSingle-quote shell strings to expand variables inside the profile environment.\nA single program name still uses the shell; '--' does not change that rule."
    )]
    Exec {
        /// Existing profile name.
        name: String,
        /// One argument runs through the system shell; multiple arguments execute directly.
        #[arg(
            value_name = "COMMAND",
            required = true,
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    /// Create a new profile with only safe display/model preferences from another.
    #[command(
        after_help = "Copies only string theme, model, and small_model settings with the default schema.\nPlugins, provider settings, credentials, and data are not copied.\n\nExample: opencode-profile clone hp work"
    )]
    Clone {
        /// Existing profile to read safe preferences from.
        source: String,
        /// New profile name (must not already exist).
        destination: String,
    },
    /// Delete a profile's config, data, state, and cache directories.
    #[command(
        after_help = "Without --yes, requires an interactive terminal and typing the profile name.\n\nExample: opencode-profile remove work --yes"
    )]
    Remove {
        /// Profile to permanently delete.
        name: String,
        /// Skip interactive confirmation (required in scripts).
        #[arg(long)]
        yes: bool,
    },
    /// Show OpenCode's location, profile roots, and readiness (not authentication).
    Doctor,
}

#[derive(Clone, Copy, Subcommand)]
pub(crate) enum ServiceAction {
    /// Manage Linux systemd user autostart (does not configure lingering).
    Autostart {
        #[command(subcommand)]
        action: AutostartAction,
    },
    /// Start or reuse the profile service.
    Start,
    /// Report status without starting a stopped service.
    Status,
    /// Stop the profile service (already stopped is a success).
    Stop,
    /// Stop then start, applying the current startup environment.
    Restart,
}

#[derive(Clone, Copy, Subcommand)]
pub(crate) enum AutostartAction {
    /// Enable and start the user unit.
    Enable,
    /// Disable future starts without stopping the running service.
    Disable,
    /// Query the user manager without starting anything.
    Status,
}
