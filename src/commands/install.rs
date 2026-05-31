//! Install a PHP version into `~/.pow/php/{version}`.
//!
//! Two strategies, chosen per-platform by `php::strategy`:
//!   * Prebuilt — download a static binary and extract it (PHP 8.x, Windows).
//!   * Source   — download the php.net tarball and compile it (PHP 7.4 on
//!                Linux/macOS, where no static build exists).

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{config, php, utils};

/// Install the PHP version described by `target` (e.g. `php@7.4`).
pub async fn run(target: &str) -> Result<()> {
    let version = php::parse_version(target)?;
    let dir = config::php_dir(&version)?;

    if config::php_bin(&version)?.exists() {
        utils::warn(&format!("PHP {version} is already installed."));
        return Ok(());
    }

    match php::strategy(&version)? {
        php::Install::Prebuilt(url) => {
            utils::info(&format!("Installing PHP {version} (prebuilt)…"));
            utils::download_and_extract(&url, &dir).await?;
            make_executable(&config::php_bin(&version)?);
        }
        php::Install::Source { url, patch } => {
            build_from_source(&url, &patch, &version, &dir).await?;
        }
    }

    if !config::php_bin(&version)?.exists() {
        bail!(
            "install finished but no PHP binary was found at {}.",
            config::php_bin(&version)?.display()
        );
    }

    utils::success(&format!("PHP {version} installed at {}", dir.display()));
    utils::info(&format!("Switch to it with `pow use php@{version}`."));
    Ok(())
}

/// Extensions Laravel needs that are *not* enabled by default in a 7.4 build.
/// pdo, tokenizer, xml, ctype and json are already on by default in PHP 7.4.
const CONFIGURE_FLAGS: &[&str] = &[
    "--disable-cgi",
    "--without-pear",
    // opcache is the only shared (zend_extension) module here, and its `.la`
    // build rule is incompatible with GNU Make 4.4+. Disable it — it's a perf
    // cache, not needed for correctness, and undesirable in a dev workflow.
    "--disable-opcache",
    "--enable-mbstring",
    "--with-openssl",
    "--with-zlib",
    "--with-pdo-sqlite",
    "--with-pdo-mysql=mysqlnd",
    "--enable-bcmath",
    "--enable-sockets",
    "--enable-fileinfo",
];

/// OpenSSL version vendored from source when the system has OpenSSL 3.x.
/// PHP 7.4's `ext/openssl` does not compile against OpenSSL 3.
const OPENSSL_VERSION: &str = "1.1.1w";

/// libxml2 version vendored when the system libxml2 is too new (>= 2.12).
/// PHP 7.4's `ext/libxml` uses APIs removed/changed in libxml2 2.12+.
const LIBXML2_VERSION: &str = "2.9.14";

/// OpenSSL source tarball (GitHub release).
fn openssl_url() -> String {
    let tag = OPENSSL_VERSION.replace('.', "_");
    format!("https://github.com/openssl/openssl/releases/download/OpenSSL_{tag}/openssl-{OPENSSL_VERSION}.tar.gz")
}

/// libxml2 source tarball (GNOME release, ships a ready `./configure`).
fn libxml2_url() -> String {
    let minor = LIBXML2_VERSION.rsplit_once('.').map(|(m, _)| m).unwrap_or("2.9");
    format!("https://download.gnome.org/sources/libxml2/{minor}/libxml2-{LIBXML2_VERSION}.tar.xz")
}

/// Download the source tarball and compile PHP into `dir` (the install prefix).
async fn build_from_source(url: &str, patch: &str, version: &str, dir: &Path) -> Result<()> {
    utils::info(&format!(
        "No prebuilt PHP {version} for this platform — building from source."
    ));

    // On a modern system some of PHP 7.4's bundled extensions can't compile
    // against the latest system libraries (OpenSSL 3, libxml2 2.12+). Build the
    // incompatible ones from source and link PHP against those instead.
    let mut vendored: Vec<PathBuf> = Vec::new();
    if let Some(p) = ensure_openssl(version).await? {
        vendored.push(p);
    }
    if let Some(p) = ensure_libxml2(version).await? {
        vendored.push(p);
    }

    let build_root = config::pow_home()?.join("build");
    config::ensure_dir(&build_root)?;
    utils::download_and_extract(url, &build_root).await?;

    let src_dir = build_root.join(format!("php-{patch}"));
    if !src_dir.join("configure").exists() {
        bail!(
            "expected source at {} but it was not extracted as expected.",
            src_dir.display()
        );
    }

    let jobs = jobs();

    // ./configure --prefix=<dir> <flags...>, pointed at vendored OpenSSL if any.
    utils::info("Configuring PHP…");
    let mut configure = Command::new("./configure");
    configure
        .arg(format!("--prefix={}", dir.display()))
        .args(CONFIGURE_FLAGS)
        .current_dir(&src_dir)
        // PHP 7.4 predates C23; GCC 14+ defaults to a stricter C where empty
        // parameter lists mean `(void)`, which breaks its old K&R-style code.
        // Force C17 semantics so it compiles on modern toolchains.
        .env("CFLAGS", "-std=gnu17 -O2");
    if !vendored.is_empty() {
        apply_vendored_env(&mut configure, &vendored);
    }
    run_step(&mut configure, "configure").context(
        "`./configure` failed — you may be missing build deps. \
         On Debian/Ubuntu: build-essential libxml2-dev libonig-dev \
         libsqlite3-dev zlib1g-dev. On Arch: base-devel libxml2 oniguruma sqlite zlib.",
    )?;

    utils::info(&format!("Compiling PHP with {jobs} jobs (this can take a while)…"));
    run_step(
        Command::new("make").arg("-j").arg(&jobs).current_dir(&src_dir),
        "make",
    )?;

    utils::info("Installing…");
    run_step(
        Command::new("make").arg("install").current_dir(&src_dir),
        "make install",
    )?;
    // `make install` places the CLI at <prefix>/bin/php, which `config::php_bin`
    // resolves directly — no symlink needed (and <prefix>/php is a directory).

    // Reclaim disk: the extracted source tree is large and no longer needed.
    let _ = std::fs::remove_dir_all(&src_dir);
    Ok(())
}

/// Ensure an OpenSSL 1.1 the PHP 7.x build can link against.
///
/// Returns `None` when nothing special is needed (non-7.x, Windows, or the
/// system already ships OpenSSL 1.1). Otherwise builds OpenSSL from source into
/// `~/.pow/deps/openssl-1.1` and returns that prefix.
async fn ensure_openssl(version: &str) -> Result<Option<PathBuf>> {
    if !version.starts_with("7.") || cfg!(windows) {
        return Ok(None);
    }
    // System OpenSSL already 1.1.x? Use it as-is.
    if let Some(v) = pkgconf_modversion("openssl") {
        if v.starts_with("1.1") {
            return Ok(None);
        }
    }

    let prefix = config::deps_dir()?.join("openssl-1.1");
    if openssl_pkgconfig(&prefix).exists() {
        utils::info("Using previously built OpenSSL 1.1.");
        return Ok(Some(prefix));
    }

    build_openssl(&prefix).await?;
    Ok(Some(prefix))
}

/// Download + compile OpenSSL (shared) from source into `prefix`.
async fn build_openssl(prefix: &Path) -> Result<()> {
    utils::info(&format!(
        "System OpenSSL is 3.x — building OpenSSL {OPENSSL_VERSION} from source (PHP 7.4 needs 1.1)."
    ));

    let build_root = config::pow_home()?.join("build");
    config::ensure_dir(&build_root)?;
    utils::download_and_extract(&openssl_url(), &build_root).await?;

    let src = build_root.join(format!("openssl-{OPENSSL_VERSION}"));
    if !src.join("config").exists() {
        bail!("OpenSSL source not extracted at {}", src.display());
    }

    let jobs = jobs();
    utils::info("Configuring OpenSSL…");
    run_step(
        Command::new("./config")
            .arg(format!("--prefix={}", prefix.display()))
            .arg(format!("--openssldir={}", prefix.display()))
            .args(["shared", "no-tests"])
            .current_dir(&src),
        "openssl ./config",
    )?;

    utils::info(&format!("Compiling OpenSSL with {jobs} jobs…"));
    run_step(
        Command::new("make").arg("-j").arg(&jobs).current_dir(&src),
        "openssl make",
    )?;

    // install_sw = libraries + headers only (skip the slow man-page install).
    run_step(
        Command::new("make").arg("install_sw").current_dir(&src),
        "openssl make install_sw",
    )?;

    if !openssl_pkgconfig(prefix).exists() {
        bail!(
            "OpenSSL built but no pkg-config file at {}.",
            openssl_pkgconfig(prefix).display()
        );
    }

    // Drop the source tree, but KEEP the prefix — the shared libs are needed
    // at runtime via the rpath baked into the php binary.
    let _ = std::fs::remove_dir_all(&src);
    utils::success(&format!("OpenSSL {OPENSSL_VERSION} built at {}", prefix.display()));
    Ok(())
}

/// Ensure a libxml2 the PHP 7.x build can compile against. Returns `None` when
/// the system libxml2 is old enough (< 2.12); otherwise builds 2.9.14 from
/// source into `~/.pow/deps/libxml2`.
async fn ensure_libxml2(version: &str) -> Result<Option<PathBuf>> {
    if !version.starts_with("7.") || cfg!(windows) {
        return Ok(None);
    }
    // System libxml2 still compatible? Use it.
    if let Some(v) = pkgconf_modversion("libxml-2.0") {
        if !libxml_too_new(&v) {
            return Ok(None);
        }
    }

    let prefix = config::deps_dir()?.join("libxml2");
    if libxml2_pkgconfig(&prefix).exists() {
        utils::info("Using previously built libxml2.");
        return Ok(Some(prefix));
    }

    build_libxml2(&prefix).await?;
    Ok(Some(prefix))
}

/// Download + compile libxml2 (shared) from source into `prefix`.
async fn build_libxml2(prefix: &Path) -> Result<()> {
    utils::info(&format!(
        "System libxml2 is too new for PHP 7.4 — building libxml2 {LIBXML2_VERSION} from source."
    ));

    let build_root = config::pow_home()?.join("build");
    config::ensure_dir(&build_root)?;
    utils::download_and_extract(&libxml2_url(), &build_root).await?;

    let src = build_root.join(format!("libxml2-{LIBXML2_VERSION}"));
    if !src.join("configure").exists() {
        bail!("libxml2 source not extracted at {}", src.display());
    }

    let jobs = jobs();
    utils::info("Configuring libxml2…");
    run_step(
        Command::new("./configure")
            .arg(format!("--prefix={}", prefix.display()))
            .args(["--without-python", "--without-lzma", "--without-icu"])
            .current_dir(&src),
        "libxml2 configure",
    )?;

    utils::info(&format!("Compiling libxml2 with {jobs} jobs…"));
    run_step(
        Command::new("make").arg("-j").arg(&jobs).current_dir(&src),
        "libxml2 make",
    )?;
    run_step(
        Command::new("make").arg("install").current_dir(&src),
        "libxml2 make install",
    )?;

    if !libxml2_pkgconfig(prefix).exists() {
        bail!(
            "libxml2 built but no pkg-config file at {}.",
            libxml2_pkgconfig(prefix).display()
        );
    }
    let _ = std::fs::remove_dir_all(&src);
    utils::success(&format!("libxml2 {LIBXML2_VERSION} built at {}", prefix.display()));
    Ok(())
}

/// True for libxml2 >= 2.12 (and 3.x), which drop APIs PHP 7.4 relies on.
fn libxml_too_new(v: &str) -> bool {
    let mut it = v.split('.');
    match (it.next(), it.next()) {
        (Some(maj), Some(min)) => {
            let maj: u32 = maj.parse().unwrap_or(0);
            let min: u32 = min.parse().unwrap_or(0);
            maj > 2 || (maj == 2 && min >= 12)
        }
        _ => false,
    }
}

/// Point a `./configure` at all vendored deps: pkg-config for discovery, plus
/// an rpath so the resulting php finds each shared lib at runtime.
fn apply_vendored_env(cmd: &mut Command, prefixes: &[PathBuf]) {
    // Combined PKG_CONFIG_PATH: every <prefix>/lib/pkgconfig, then the existing.
    let mut pkg_parts: Vec<PathBuf> = prefixes
        .iter()
        .map(|p| p.join("lib").join("pkgconfig"))
        .collect();
    if let Some(existing) = std::env::var_os("PKG_CONFIG_PATH") {
        pkg_parts.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(&pkg_parts) {
        cmd.env("PKG_CONFIG_PATH", joined);
    }

    // Single rpath listing every <prefix>/lib, colon-separated.
    let rpath = prefixes
        .iter()
        .map(|p| p.join("lib").display().to_string())
        .collect::<Vec<_>>()
        .join(":");
    cmd.env("LDFLAGS", format!("-Wl,-rpath,{rpath}"));

    for p in prefixes {
        utils::info(&format!("Linking PHP against vendored lib at {}", p.display()));
    }
}

/// `<prefix>/lib/pkgconfig/openssl.pc`
fn openssl_pkgconfig(prefix: &Path) -> PathBuf {
    prefix.join("lib").join("pkgconfig").join("openssl.pc")
}

/// `<prefix>/lib/pkgconfig/libxml-2.0.pc`
fn libxml2_pkgconfig(prefix: &Path) -> PathBuf {
    prefix.join("lib").join("pkgconfig").join("libxml-2.0.pc")
}

/// `pkg-config --modversion <pkg>` for the default search path, if available.
fn pkgconf_modversion(pkg: &str) -> Option<String> {
    let out = Command::new("pkg-config")
        .args(["--modversion", pkg])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Number of parallel build jobs, as a string for `make -j`.
fn jobs() -> String {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .to_string()
}

/// Run a build step inheriting stdio so the user sees live progress, and turn a
/// non-zero exit into an error.
fn run_step(cmd: &mut Command, name: &str) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn `{name}`"))?;
    if !status.success() {
        bail!("`{name}` exited with {status}");
    }
    Ok(())
}

/// Mark a freshly extracted prebuilt binary executable on unix.
fn make_executable(bin: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if bin.exists() {
            if let Ok(meta) = std::fs::metadata(bin) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(bin, perms);
            }
        }
    }
    #[cfg(not(unix))]
    let _ = bin;
}
