# POW — PHP One-click Workspace

Ultra-minimal, cross-platform PHP development environment written in Rust.
Think XAMPP, but just PHP + SQLite/MariaDB — no bloat.

## Install

### One-liner (Linux / macOS)

```sh
curl -fsSL https://raw.githubusercontent.com/AryAntara/pow-php/main/install.sh | sh
```

This grabs a prebuilt binary for your platform from the
[latest release](https://github.com/AryAntara/pow-php/releases). If no release
is available (or your platform isn't prebuilt), it automatically falls back to
building from source with `cargo`.

### With cargo

```sh
cargo install --git https://github.com/AryAntara/pow-php --locked
```

### Windows

Download `pow-x86_64-pc-windows-msvc.zip` from the
[releases page](https://github.com/AryAntara/pow-php/releases), unzip it, and
put `pow.exe` somewhere on your `PATH`.

### From source

```sh
git clone https://github.com/AryAntara/pow-php
cd pow-php
cargo build --release        # binary at target/release/pow
cargo install --path .       # or install it onto your PATH
```

## Usage

```sh
pow install php@8.2     # install a PHP version (prebuilt, or built from source)
pow ls                  # list installed PHP versions
pow use php@8.2         # set the active version
pow start               # start the PHP built-in server
pow stop                # stop it
pow restart
pow status              # status of all services
pow shell               # subshell with the active PHP on PATH (exit to leave)

pow db sqlite           # use SQLite (default, file-based)
pow db mysql            # use bundled MariaDB
pow db start | stop
pow db backup           # back up the active DB to ~/.pow/backups/
pow db restore <file>   # restore from a backup
```

## PHP versions

Supported: **7.4, 8.1, 8.2, 8.3** (Laravel 5.8 → 11.x).

* **8.x** installs as a prebuilt static binary (all platforms).
* **7.4** on Linux/macOS is compiled from source and is fully self-contained:
  POW vendors OpenSSL 1.1 and libxml2 from source when the system ships newer,
  incompatible versions, so it builds even on bleeding-edge toolchains. On
  Windows, 7.4 uses the official windows.php.net build.

All data lives under `~/.pow/`.

## Releasing (maintainers)

Push a version tag — GitHub Actions builds all four targets and publishes them:

```sh
git tag v0.1.0
git push origin v0.1.0
```

## License

MIT
