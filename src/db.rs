//! MariaDB download-URL resolution and binary-path helpers.
//!
//! SQLite needs no install — it ships inside PHP via `pdo_sqlite`, so this
//! module only concerns itself with the optional MariaDB bundle.

use anyhow::{bail, Result};

/// Pinned MariaDB release used for the bundled server.
pub const MARIADB_VERSION: &str = "11.4.2";

/// Resolve the MariaDB download URL for the current platform.
pub fn download_url() -> Result<String> {
    let v = MARIADB_VERSION;
    let url = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => format!(
            "https://archive.mariadb.org/mariadb-{v}/bintar-linux-systemd-x86_64/mariadb-{v}-linux-systemd-x86_64.tar.gz"
        ),
        ("macos", _) => format!(
            "https://archive.mariadb.org/mariadb-{v}/bintar-macos-x86_64/mariadb-{v}-macos-x86_64.tar.gz"
        ),
        ("windows", "x86_64") => format!(
            "https://archive.mariadb.org/mariadb-{v}/winx64-packages/mariadb-{v}-winx64.zip"
        ),
        (os, arch) => bail!("no MariaDB build available for {os}/{arch}"),
    };
    Ok(url)
}
