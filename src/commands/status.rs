//! Show the status of every POW service at a glance.

use anyhow::Result;
use colored::Colorize;

use crate::{config, utils};

/// `pow status` — print the active config plus PHP/DB server state.
pub fn run() -> Result<()> {
    let cfg = config::load()?;

    println!("{}", "POW status".bold().underline());
    println!();

    // PHP install + server.
    let installed = config::php_bin(&cfg.php_version)?.exists();
    let php_state = if installed {
        "installed".green()
    } else {
        "not installed".red()
    };
    println!("  PHP version : {} ({})", cfg.php_version.cyan(), php_state);

    match server_pid_state(&config::php_pid_file()?) {
        Some(pid) => println!("  PHP server  : {} (pid {pid})", "running".green()),
        None => println!("  PHP server  : {}", "stopped".dimmed()),
    }
    println!(
        "  Document root: {}  •  port {}",
        cfg.root.cyan(),
        cfg.port.to_string().cyan()
    );

    println!();

    // Database.
    println!("  DB driver   : {}", cfg.database.driver.cyan());
    if cfg.database.driver == "mysql" {
        match server_pid_state(&config::mysql_pid_file()?) {
            Some(pid) => println!("  MariaDB     : {} (pid {pid})", "running".green()),
            None => println!("  MariaDB     : {}", "stopped".dimmed()),
        }
        println!("  MySQL port  : {}", cfg.database.mysql_port.to_string().cyan());
    } else {
        println!("  SQLite      : {} (file-based, no server)", "ready".green());
    }

    Ok(())
}

/// Return the PID only if the pid-file exists *and* the process is alive,
/// cleaning up stale pid-files as a side effect.
fn server_pid_state(pid_file: &std::path::Path) -> Option<u32> {
    let pid = utils::read_pid(pid_file)?;
    if utils::is_running(pid) {
        Some(pid)
    } else {
        utils::clear_pid(pid_file);
        None
    }
}
