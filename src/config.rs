//! Global configuration plus every path helper used across the tool.
//!
//! Nothing else in the codebase should build a `~/.pow/...` path by hand —
//! always go through the helpers here so the layout stays in one place.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Database section of the config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    /// Active driver: `sqlite` or `mysql` (MariaDB).
    pub driver: String,
    pub mysql_port: u16,
    pub mysql_user: String,
    pub mysql_pass: String,
    /// Path to the SQLite database file (used by backup/restore).
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,
}

fn default_sqlite_path() -> String {
    "./database/database.sqlite".to_string()
}

impl Default for Database {
    fn default() -> Self {
        Self {
            driver: "sqlite".to_string(),
            mysql_port: 3306,
            mysql_user: "root".to_string(),
            mysql_pass: String::new(),
            sqlite_path: default_sqlite_path(),
        }
    }
}

/// Top-level config, serialized to `~/.pow/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub root: String,
    pub php_version: String,
    #[serde(default)]
    pub database: Database,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            root: "./htdocs".to_string(),
            php_version: "8.2".to_string(),
            database: Database::default(),
        }
    }
}

/// Root of all POW data: `~/.pow`.
pub fn pow_home() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".pow"))
}

/// `~/.pow/config.json`
pub fn config_path() -> Result<PathBuf> {
    Ok(pow_home()?.join("config.json"))
}

/// `~/.pow/php` — parent of all installed PHP versions.
pub fn php_root() -> Result<PathBuf> {
    Ok(pow_home()?.join("php"))
}

/// `~/.pow/php/{version}`
pub fn php_dir(version: &str) -> Result<PathBuf> {
    Ok(php_root()?.join(version))
}

/// Path to the PHP binary for `version`, accounting for the OS extension.
///
/// Prebuilt archives drop `php` at the version root, while a source build
/// (`make install`) puts it under `bin/`. Prefer `bin/php` when present so both
/// layouts resolve to a real binary (the source layout also creates a `php/`
/// *directory* at the root, which must never be mistaken for the executable).
pub fn php_bin(version: &str) -> Result<PathBuf> {
    let exe = if cfg!(windows) { "php.exe" } else { "php" };
    let dir = php_dir(version)?;
    let in_bin = dir.join("bin").join(exe);
    if in_bin.is_file() {
        return Ok(in_bin);
    }
    Ok(dir.join(exe))
}

/// `~/.pow/db/mariadb`
pub fn db_dir() -> Result<PathBuf> {
    Ok(pow_home()?.join("db").join("mariadb"))
}

/// Path to a MariaDB `bin/` executable, accounting for the OS extension.
pub fn db_bin(name: &str) -> Result<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    Ok(db_dir()?.join("bin").join(exe))
}

/// Path to the `mysqld` server binary.
pub fn mysqld_bin() -> Result<PathBuf> {
    db_bin("mysqld")
}

/// Path to the `mysqldump` client (used by `pow db backup`).
pub fn mysqldump_bin() -> Result<PathBuf> {
    db_bin("mysqldump")
}

/// Path to the `mysql` client (used by `pow db restore`).
pub fn mysql_client_bin() -> Result<PathBuf> {
    db_bin("mysql")
}

/// `~/.pow/data/mysql` — MariaDB data directory.
pub fn mysql_data_dir() -> Result<PathBuf> {
    Ok(pow_home()?.join("data").join("mysql"))
}

/// `~/.pow/backups` — where `pow db backup` writes by default.
pub fn backups_dir() -> Result<PathBuf> {
    Ok(pow_home()?.join("backups"))
}

/// `~/.pow/deps` — vendored build dependencies (e.g. OpenSSL 1.1 for PHP 7.4).
pub fn deps_dir() -> Result<PathBuf> {
    Ok(pow_home()?.join("deps"))
}

/// PID file for the running PHP server.
pub fn php_pid_file() -> Result<PathBuf> {
    Ok(pow_home()?.join("php.pid"))
}

/// PID file for the running MariaDB server.
pub fn mysql_pid_file() -> Result<PathBuf> {
    Ok(pow_home()?.join("mysql.pid"))
}

/// Ensure `~/.pow` (and the given subdir, if any) exists.
pub fn ensure_dir(path: &PathBuf) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    Ok(())
}

/// Load the config, creating a default one on first run.
pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        let cfg = Config::default();
        save(&cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config at {}", path.display()))?;
    let cfg: Config = serde_json::from_str(&raw).context("config.json is not valid JSON")?;
    Ok(cfg)
}

/// Persist the config to disk, creating `~/.pow` if needed.
pub fn save(cfg: &Config) -> Result<()> {
    let home = pow_home()?;
    ensure_dir(&home)?;
    let raw = serde_json::to_string_pretty(cfg).context("failed to serialize config")?;
    fs::write(config_path()?, raw).context("failed to write config.json")?;
    Ok(())
}
