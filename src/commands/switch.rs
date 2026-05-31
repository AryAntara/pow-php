//! Switch the active PHP version recorded in the config.

use anyhow::{bail, Result};

use crate::{config, php, utils};

/// Set `target` (e.g. `php@8.3`) as the active PHP version.
pub fn run(target: &str) -> Result<()> {
    let version = php::parse_version(target)?;

    if !config::php_bin(&version)?.exists() {
        bail!(
            "PHP {version} is not installed. Run `pow install php@{version}` first."
        );
    }

    let mut cfg = config::load()?;
    if cfg.php_version == version {
        utils::warn(&format!("PHP {version} is already the active version."));
        return Ok(());
    }

    cfg.php_version = version.clone();
    config::save(&cfg)?;
    utils::success(&format!("Now using PHP {version}."));
    utils::info("Run `pow restart` to apply it to a running server.");
    Ok(())
}
