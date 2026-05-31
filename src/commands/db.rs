//! Database control: start / stop the server, switch the active driver, and
//! back up / restore the active database.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{config, db, utils};

/// `pow db start` — start the database for the active driver.
pub async fn start() -> Result<()> {
    let cfg = config::load()?;
    match cfg.database.driver.as_str() {
        "sqlite" => {
            utils::info("Active driver is SQLite — no server to start.");
            utils::info("SQLite databases are plain files served directly by PHP.");
            Ok(())
        }
        "mysql" => start_mysql(&cfg).await,
        other => bail!("unknown database driver '{other}'"),
    }
}

/// `pow db stop` — stop the MariaDB server, if running.
pub fn stop() -> Result<()> {
    let pid_file = config::mysql_pid_file()?;
    match utils::read_pid(&pid_file) {
        Some(pid) => {
            utils::kill_pid(pid)?;
            utils::clear_pid(&pid_file);
            utils::success(&format!("MariaDB stopped (pid {pid})."));
        }
        None => utils::warn("MariaDB is not running."),
    }
    Ok(())
}

/// `pow db sqlite` — switch the active driver to SQLite.
pub fn use_sqlite() -> Result<()> {
    switch_driver("sqlite")
}

/// `pow db mysql` — switch the active driver to MariaDB.
pub fn use_mysql() -> Result<()> {
    switch_driver("mysql")
}

fn switch_driver(driver: &str) -> Result<()> {
    let mut cfg = config::load()?;
    if cfg.database.driver == driver {
        utils::warn(&format!("Database driver is already '{driver}'."));
        return Ok(());
    }
    cfg.database.driver = driver.to_string();
    config::save(&cfg)?;
    utils::success(&format!("Database driver switched to '{driver}'."));
    Ok(())
}

async fn start_mysql(cfg: &config::Config) -> Result<()> {
    let pid_file = config::mysql_pid_file()?;
    if let Some(pid) = utils::read_pid(&pid_file) {
        if utils::is_running(pid) {
            utils::warn(&format!("MariaDB is already running (pid {pid})."));
            return Ok(());
        }
        utils::clear_pid(&pid_file);
    }

    let mysqld = config::mysqld_bin()?;
    if !mysqld.exists() {
        utils::info(&format!("MariaDB {} not found — downloading…", db::MARIADB_VERSION));
        let url = db::download_url()?;
        utils::download_and_extract(&url, &config::db_dir()?).await?;
        if !mysqld.exists() {
            bail!(
                "MariaDB downloaded but `mysqld` was not found at {}. \
                 The archive layout may differ — check {}.",
                mysqld.display(),
                config::db_dir()?.display()
            );
        }
    }

    let data_dir = config::mysql_data_dir()?;
    config::ensure_dir(&data_dir)?;

    let child = Command::new(&mysqld)
        .arg(format!("--datadir={}", data_dir.display()))
        .arg(format!("--port={}", cfg.database.mysql_port))
        .spawn()
        .with_context(|| format!("failed to launch {}", mysqld.display()))?;

    utils::write_pid(&pid_file, child.id())?;
    utils::success(&format!(
        "MariaDB started on port {} (pid {}).",
        cfg.database.mysql_port,
        child.id()
    ));
    Ok(())
}

/// `pow db backup [--out FILE]` — back up the active database.
///
/// MySQL → `mysqldump --all-databases` into a `.sql` file.
/// SQLite → a straight copy of the database file.
pub fn backup(out: Option<&str>) -> Result<()> {
    let cfg = config::load()?;
    let dest = match out {
        Some(p) => PathBuf::from(p),
        None => {
            let ext = if cfg.database.driver == "mysql" { "sql" } else { "sqlite" };
            let dir = config::backups_dir()?;
            config::ensure_dir(&dir)?;
            dir.join(format!("{}-{}.{ext}", cfg.database.driver, timestamp()))
        }
    };

    match cfg.database.driver.as_str() {
        "sqlite" => backup_sqlite(&cfg, &dest),
        "mysql" => backup_mysql(&cfg, &dest),
        other => bail!("unknown database driver '{other}'"),
    }
}

/// `pow db restore FILE` — restore the active database from a backup file.
pub fn restore(file: &str) -> Result<()> {
    let cfg = config::load()?;
    let src = PathBuf::from(file);
    if !src.exists() {
        bail!("backup file not found: {}", src.display());
    }

    match cfg.database.driver.as_str() {
        "sqlite" => restore_sqlite(&cfg, &src),
        "mysql" => restore_mysql(&cfg, &src),
        other => bail!("unknown database driver '{other}'"),
    }
}

fn backup_sqlite(cfg: &config::Config, dest: &Path) -> Result<()> {
    let src = PathBuf::from(&cfg.database.sqlite_path);
    if !src.exists() {
        bail!(
            "SQLite database not found at {}. Set `database.sqlite_path` in config.json.",
            src.display()
        );
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::copy(&src, dest)
        .with_context(|| format!("failed to copy {} -> {}", src.display(), dest.display()))?;
    utils::success(&format!("SQLite database backed up to {}", dest.display()));
    Ok(())
}

fn restore_sqlite(cfg: &config::Config, src: &Path) -> Result<()> {
    let dest = PathBuf::from(&cfg.database.sqlite_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }
    if dest.exists() {
        utils::warn(&format!("Overwriting existing database at {}", dest.display()));
    }
    fs::copy(src, &dest)
        .with_context(|| format!("failed to copy {} -> {}", src.display(), dest.display()))?;
    utils::success(&format!("SQLite database restored to {}", dest.display()));
    Ok(())
}

fn backup_mysql(cfg: &config::Config, dest: &Path) -> Result<()> {
    let bin = config::mysqldump_bin()?;
    if !bin.exists() {
        bail!("mysqldump not found at {}. Run `pow db start` once to install MariaDB.", bin.display());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }

    let file = fs::File::create(dest)
        .with_context(|| format!("failed to create {}", dest.display()))?;
    let status = mysql_cmd(&bin, &cfg.database)
        .arg("--all-databases")
        .stdout(file)
        .status()
        .context("failed to run mysqldump")?;
    if !status.success() {
        let _ = fs::remove_file(dest);
        bail!("mysqldump failed — is MariaDB running (`pow db start`)?");
    }
    utils::success(&format!("MySQL databases backed up to {}", dest.display()));
    Ok(())
}

fn restore_mysql(cfg: &config::Config, src: &Path) -> Result<()> {
    let bin = config::mysql_client_bin()?;
    if !bin.exists() {
        bail!("mysql client not found at {}. Run `pow db start` once to install MariaDB.", bin.display());
    }

    let file = fs::File::open(src)
        .with_context(|| format!("failed to open {}", src.display()))?;
    utils::warn("Restoring will overwrite matching databases in MariaDB.");
    let status = mysql_cmd(&bin, &cfg.database)
        .stdin(file)
        .status()
        .context("failed to run mysql client")?;
    if !status.success() {
        bail!("restore failed — is MariaDB running (`pow db start`)?");
    }
    utils::success(&format!("MySQL databases restored from {}", src.display()));
    Ok(())
}

/// Build a `mysql`/`mysqldump` invocation with shared connection flags.
fn mysql_cmd(bin: &Path, db: &config::Database) -> Command {
    let mut cmd = Command::new(bin);
    cmd.arg("-h")
        .arg("127.0.0.1")
        .arg("-P")
        .arg(db.mysql_port.to_string())
        .arg("-u")
        .arg(&db.mysql_user);
    // MySQL CLI tools want the password glued to `-p` with no space.
    if !db.mysql_pass.is_empty() {
        cmd.arg(format!("-p{}", db.mysql_pass));
    }
    cmd
}

/// Seconds since the Unix epoch, for unique backup filenames.
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
