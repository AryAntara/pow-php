//! start / stop / restart of the PHP built-in web server.

use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::{config, utils};

/// Start the PHP built-in server using the active version and config.
pub fn start() -> Result<()> {
    let cfg = config::load()?;

    // Refuse to start twice.
    let pid_file = config::php_pid_file()?;
    if let Some(pid) = utils::read_pid(&pid_file) {
        if utils::is_running(pid) {
            utils::warn(&format!("PHP server is already running (pid {pid})."));
            return Ok(());
        }
        utils::clear_pid(&pid_file);
    }

    let php = config::php_bin(&cfg.php_version)?;
    if !php.exists() {
        bail!(
            "PHP {} is not installed. Run `pow install php@{}` first.",
            cfg.php_version,
            cfg.php_version
        );
    }

    let addr = format!("127.0.0.1:{}", cfg.port);
    let child = Command::new(&php)
        .arg("-S")
        .arg(&addr)
        .arg("-t")
        .arg(&cfg.root)
        .spawn()
        .with_context(|| format!("failed to launch {}", php.display()))?;

    utils::write_pid(&pid_file, child.id())?;
    utils::success(&format!(
        "PHP {} server started on http://{} (root: {}, pid {})",
        cfg.php_version,
        addr,
        cfg.root,
        child.id()
    ));
    Ok(())
}

/// Stop the running PHP server, if any.
pub fn stop() -> Result<()> {
    let pid_file = config::php_pid_file()?;
    match utils::read_pid(&pid_file) {
        Some(pid) => {
            utils::kill_pid(pid)?;
            utils::clear_pid(&pid_file);
            utils::success(&format!("PHP server stopped (pid {pid})."));
        }
        None => utils::warn("No PHP server is running."),
    }
    Ok(())
}

/// Restart: stop (ignoring "not running") then start.
pub fn restart() -> Result<()> {
    let _ = stop();
    start()
}
