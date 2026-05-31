# POW — PHP One-click Workspace

## Project Overview

POW is a lightweight, cross-platform PHP development environment built with Rust.
Think XAMPP but ultra-minimal — just PHP + SQLite/MariaDB, no bloat.

## Tech Stack

* **Language** : Rust (stable)
* **CLI** : `clap` v4 with derive macros
* **Async** : `tokio`
* **HTTP/Download** : `reqwest`
* **Config** : `serde_json`
* **UI** : `colored` + `indicatif`

## Project Structure

```
src/
├── main.rs              # Entry point + CLI command routing
├── config.rs            # Config struct, load/save, path helpers
├── php.rs               # PHP version parsing + install strategy (prebuilt/source) + URLs
├── db.rs                # MariaDB download URL + binary path
├── utils.rs             # Logging + shared download/extract + process helpers
└── commands/
    ├── mod.rs
    ├── server.rs        # start / stop / restart PHP server
    ├── install.rs       # Install PHP (download prebuilt, or build from source + vendored deps)
    ├── list.rs          # List installed PHP versions (pow ls)
    ├── switch.rs        # Switch active PHP version
    ├── shell.rs         # Enter a subshell with a PHP version on PATH
    ├── db.rs            # Database start/stop/switch
    └── status.rs        # Show status of all services
```

## Coding Principles

* **DRY** — Never repeat logic. Shared helpers go in `config.rs` or `utils.rs`
* **KISS** — Keep each function simple and focused
* **Modular** — One responsibility per file/module
* **No unwrap()** — Always use `?` or proper error handling with `anyhow`
* **No unsafe** — Avoid unsafe Rust unless absolutely necessary

## Conventions

* Use `anyhow::Result` for all error handling
* All user-facing messages go through `utils::{success, info, warn, error}`
* Path logic always goes through `config::pow_home()` and related helpers
* PHP version strings are always normalized via `php::parse_version()`
* Config is always loaded via `config::load()` and saved via `config::save()`

## Config Location

All POW data lives in the user's home directory:

```
~/.pow/
├── config.json          # Global config
├── php/{version}/       # PHP binaries
├── db/mariadb/          # MariaDB binaries
└── data/mysql/          # MariaDB data files
```

## Config Schema

```json
{
  "port": 8080,
  "root": "./htdocs",
  "php_version": "8.2",
  "database": {
    "driver": "sqlite",
    "mysql_port": 3306,
    "mysql_user": "root",
    "mysql_pass": "",
    "sqlite_path": "./database/database.sqlite"
  }
}
```

## Available Commands

```bash
pow start                # Start PHP built-in server
pow stop                 # Stop PHP server
pow restart              # Restart PHP server
pow status               # Show status of all services
pow install php@8.2      # Install PHP version (prebuilt binary, or build from source)
pow ls                   # List installed PHP versions (alias: pow list)
pow use php@8.2          # Switch active PHP version
pow shell                # Enter a subshell with the active PHP on PATH (exit to leave)
pow shell php@8.3        # Enter a subshell with a specific PHP version
pow db start             # Start database service
pow db stop              # Stop database service
pow db sqlite            # Switch to SQLite
pow db mysql             # Switch to MariaDB
pow db backup            # Back up active DB to ~/.pow/backups/
pow db backup --out f    # Back up to a specific file
pow db restore <file>    # Restore active DB from a backup file
```

## Cross-Platform Notes

* Windows binary: `php.exe`, `mysqld.exe`
* Linux/Mac binary: `php`, `mysqld`
* Use `cfg!(windows)` for OS-specific logic
* Use `std::env::consts::OS` and `std::env::consts::ARCH` for runtime detection
* Cross-compile targets:
  * Windows:        `x86_64-pc-windows-gnu`
  * Linux:          `x86_64-unknown-linux-gnu`
  * Mac Intel:      `x86_64-apple-darwin`
  * Mac ARM:        `aarch64-apple-darwin`

## Laravel Compatibility

PHP versions supported: 7.4, 8.1, 8.2, 8.3
Required extensions: mbstring, openssl, pdo, pdo_sqlite, pdo_mysql, tokenizer, xml, ctype, json, bcmath

Laravel ↔ PHP matrix:

* Laravel 5.8:  PHP 7.4
* Laravel 9.x:  PHP 8.1
* Laravel 10.x: PHP 8.1–8.3
* Laravel 11.x: PHP 8.2–8.3

### Install strategy

PHP 8.x: prebuilt static binaries from static-php.dev (all platforms) and
windows.php.net (Windows). PHP 7.4 has no static build for Linux/macOS, so it is
**compiled from the php.net source tarball** on first install (Windows 7.4 still
uses the windows.php.net archive). Building 7.4 from source needs a C toolchain
plus dev headers:

* Debian/Ubuntu: `build-essential libxml2-dev libssl-dev libonig-dev libsqlite3-dev zlib1g-dev`
* Arch: `base-devel libxml2 openssl oniguruma sqlite zlib`

## When Adding New Features

1. Add new command in `src/commands/` as a new file
2. Register it in `src/commands/mod.rs`
3. Add the subcommand to `Commands` enum in `main.rs`
4. Keep shared logic in `config.rs`, `php.rs`, `db.rs`, or `utils.rs`
5. Never duplicate path logic — always use helpers from `config.rs`
