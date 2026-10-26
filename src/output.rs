use crate::profile::Roots;
use anyhow::Result;
use std::env;
use std::io::{self, IsTerminal};
use tabled::{builder::Builder, settings::Style};

pub(crate) fn color_enabled(terminal: bool) -> bool {
    terminal
        && env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && env::var_os("TERM").is_none_or(|value| value != "dumb")
}

fn color_icons(text: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    let mut colored = String::new();
    for ch in text.chars() {
        let code = match ch {
            '✓' => "32",
            '○' => "90",
            '⚠' => "33",
            '◈' | '●' | '⚙' | '▤' | '◷' | '↻' | 'ⓘ' => "36",
            _ => {
                colored.push(ch);
                continue;
            }
        };
        colored.push_str(&format!("\x1b[{code}m{ch}\x1b[0m"));
    }
    colored
}

pub(crate) fn print_output(text: &str) {
    println!(
        "{}",
        color_icons(text, color_enabled(io::stdout().is_terminal()))
    );
}

pub(crate) fn print_table(headers: [&str; 2], rows: Vec<[String; 2]>) {
    if rows.is_empty() {
        return;
    }
    let mut builder = Builder::default();
    builder.push_record(headers);
    for row in rows {
        builder.push_record(row);
    }
    // Add ANSI colors after layout so escape sequences never affect column widths.
    print_output(&builder.build().with(Style::rounded()).to_string());
}

pub(crate) fn print_services(rows: Vec<[String; 3]>) {
    if rows.is_empty() {
        print_output("No profiles found");
        return;
    }
    let mut builder = Builder::default();
    builder.push_record(["◈ Profile", "⚙ Service", "↻ Autostart"]);
    for [profile, service, autostart] in rows {
        let service = match service.as_str() {
            "running" => format!("✓ {service}"),
            "stopped" => format!("○ {service}"),
            "unhealthy" | "unknown" => format!("⚠ {service}"),
            _ => service,
        };
        let autostart = match autostart.as_str() {
            "enabled" => format!("✓ {autostart}"),
            "disabled" => format!("○ {autostart}"),
            "unavailable" => format!("⚠ {autostart}"),
            _ => autostart,
        };
        builder.push_record([profile, service, autostart]);
    }
    // Layout first so ANSI escape sequences never affect column widths.
    print_output(&builder.build().with(Style::rounded()).to_string());
}

pub(crate) fn print_profiles(roots: &Roots) -> Result<()> {
    let rows = roots
        .names()?
        .into_iter()
        .map(|name| {
            let status = roots.profile(&name)?.status().to_string();
            Ok([name, status])
        })
        .collect::<Result<Vec<_>>>()?;
    print_table(["◈ Profile", "● Status"], rows);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::color_icons;

    #[test]
    fn icons_are_colored_only_when_enabled() {
        let text = "✓ ready ⚠ incomplete ⓘ Auth ◈ Profile";
        assert_eq!(color_icons(text, false), text);
        let colored = color_icons(text, true);
        assert!(colored.contains("\x1b[32m✓\x1b[0m"));
        assert!(colored.contains("\x1b[33m⚠\x1b[0m"));
        assert!(colored.contains("\x1b[36mⓘ\x1b[0m"));
        assert!(colored.contains("\x1b[36m◈\x1b[0m"));
    }
}
