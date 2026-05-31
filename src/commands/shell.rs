//! `pow shell` — spawn a subshell with the active (or given) PHP prepended to
//! PATH. Because it's a child process, leaving it (`exit`) restores the parent
//! environment automatically: the PHP you used inside is "gone" afterwards.

use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::{config, php, utils};

/// Enter a POW shell. `target` overrides the active version (e.g. `php@8.3`).
pub fn run(target: Option<&str>) -> Result<()> {
    let cfg = config::load()?;
    let version = match target {
        Some(t) => php::parse_version(t)?,
        None => cfg.php_version.clone(),
    };

    let dir = config::php_dir(&version)?;
    if !config::php_bin(&version)?.exists() {
        bail!("PHP {version} is not installed. Run `pow install php@{version}` first.");
    }

    if let Ok(active) = std::env::var("POW_SHELL") {
        utils::warn(&format!(
            "Already inside a POW shell (PHP {active}). Type `exit` first to leave it."
        ));
    }

    // Prepend this PHP's dirs so `php` (and phpize/php-config for source builds)
    // shadow any system PHP for the lifetime of the subshell.
    let mut paths = vec![dir.clone(), dir.join("bin")];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let new_path = std::env::join_paths(paths).context("failed to assemble PATH")?;

    let shell = pick_shell();
    utils::success(&format!(
        "Entering POW shell — PHP {version} is active. Type `exit` to leave."
    ));

    let status = Command::new(&shell)
        .env("PATH", &new_path)
        .env("POW_SHELL", &version)
        .env("POW_PHP_VERSION", &version)
        .status()
        .with_context(|| format!("failed to launch shell '{shell}'"))?;

    utils::info(&format!(
        "Left POW shell — PHP {version} is no longer on PATH."
    ));
    if !status.success() {
        // A non-zero exit from an interactive shell is normal (e.g. Ctrl-D vs
        // last command status); don't treat it as a POW error.
    }
    Ok(())
}

/// Pick an interactive shell for the current platform.
fn pick_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}
