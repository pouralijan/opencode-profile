use crate::cli::{Action, Cli};
use crate::output::{print_output, print_profiles, print_table};
use crate::process::{exec, run};
use crate::profile::Roots;
use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};

pub(crate) fn execute(cli: Cli) -> Result<i32> {
    let roots = Roots::new()?;
    match cli.command {
        Action::Create { name } => {
            let profile = roots.profile(&name)?;
            profile.create()?;
            print_output(&format!("✓ Created {name}"));
        }
        Action::List => {
            print_profiles(&roots)?;
        }
        Action::Services => {
            let mut rows = Vec::new();
            for name in roots.names()? {
                let profile = roots.profile(&name)?;
                rows.push([
                    name,
                    crate::service::snapshot(&profile).into(),
                    crate::autostart::snapshot(&profile).into(),
                ]);
            }
            crate::output::print_services(rows);
        }
        Action::Show { name } => {
            let profile = roots.profile(&name)?;
            if !profile.exists() {
                bail!("Profile '{name}' not found");
            }
            print_table(
                ["Field", "Value"],
                vec![
                    ["◈ Name".into(), name],
                    ["● Status".into(), profile.status().into()],
                    ["⚙ Config".into(), profile.config.display().to_string()],
                    ["▤ Data".into(), profile.data.display().to_string()],
                    ["◷ State".into(), profile.state.display().to_string()],
                    ["↻ Cache".into(), profile.cache.display().to_string()],
                    [
                        "⚙ CLI preferences".into(),
                        profile
                            .config
                            .join(".xdg/opencode/cli.json")
                            .display()
                            .to_string(),
                    ],
                ],
            );
        }
        Action::Run { name, args } => return run(&roots.profile(&name)?, args),
        Action::Exec { name, args } => return exec(&roots.profile(&name)?, args),
        Action::Service { name, action } => {
            return crate::service::operate(&roots.profile(&name)?, action);
        }
        Action::Clone {
            source,
            destination,
        } => {
            let src = roots.profile(&source)?;
            src.require_config()?;
            let dst = roots.profile(&destination)?;
            if dst.exists() {
                bail!("Profile '{destination}' already exists");
            }
            let settings = src.safe_settings()?;
            dst.create()?;
            fs::write(
                dst.config.join("opencode.json"),
                serde_json::to_vec_pretty(&settings)?,
            )?;
            print_output(&format!("✓ Cloned safe settings from {source} to {destination}; credentials and data were not copied"));
        }
        Action::Remove { name, yes } => {
            let profile = roots.profile(&name)?;
            if !profile.exists() {
                bail!("Profile '{name}' not found");
            }
            if !yes {
                if !io::stdin().is_terminal() {
                    bail!("Removal requires --yes without an interactive terminal");
                }
                print!("Delete profile '{name}' config, data, state and cache? Type profile name to confirm: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if input.trim() != name {
                    bail!("Removal cancelled");
                }
            }
            crate::autostart::remove_profile(&profile)?;
            print_output(&format!("✓ Removed {name}"));
        }
        Action::Doctor => {
            let found = env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
                .map(|dir| dir.join("opencode"))
                .find(|path| path.is_file());
            print_table(
                ["Check", "Value"],
                vec![
                    [
                        "⚙ opencode".into(),
                        found.map_or_else(
                            || "⚠ not found in PATH".to_string(),
                            |p| p.display().to_string(),
                        ),
                    ],
                    ["⚙ Config root".into(), roots.config.display().to_string()],
                    ["▤ Data root".into(), roots.data.display().to_string()],
                    ["◷ State root".into(), roots.state.display().to_string()],
                    ["↻ Cache root".into(), roots.cache.display().to_string()],
                ],
            );
            print_profiles(&roots)?;
            print_output("ⓘ Auth: unverified; check inside each profile with `opencode-profile run NAME auth list`");
        }
    }
    Ok(0)
}
