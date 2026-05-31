//! `pow ls` — list installed PHP versions and the installable ones.

use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::{config, php, utils};

/// List installed PHP versions, marking the active one.
pub fn run() -> Result<()> {
    let cfg = config::load()?;
    let root = config::php_root()?;

    println!("{}", "Installed PHP versions".bold().underline());

    let mut installed: Vec<String> = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(&root)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Only count it if the binary actually resolved (skip junk dirs).
            if config::php_bin(&name)?.exists() {
                installed.push(name);
            }
        }
    }
    installed.sort();

    if installed.is_empty() {
        utils::warn("No PHP versions installed yet. Try `pow install php@8.2`.");
    } else {
        for v in &installed {
            if *v == cfg.php_version {
                println!("  {} {}{}", "*".green().bold(), v.green().bold(), " (active)".green());
            } else {
                println!("    {v}");
            }
        }
    }

    println!();
    println!("{}", "Installable".bold());
    println!("  {}", php::supported_versions().join(", "));
    Ok(())
}
