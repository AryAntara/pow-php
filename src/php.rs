//! PHP version normalization and download-URL resolution.
//!
//! Static, self-contained PHP CLI builds are pulled from the static-php-cli
//! project (https://dl.static-php.dev) for Linux/macOS, and from
//! windows.php.net for Windows. Only the URL logic lives here; the actual
//! download/extract is handled by `utils::download_and_extract`.

use anyhow::{bail, Result};

/// PHP minor versions we know how to install, pinned to a known-good patch
/// release. Keep this list aligned with the "Laravel Compatibility" section
/// of CLAUDE.md (7.4 for Laravel 5.8; 8.1 / 8.2 / 8.3 for Laravel 9+).
const KNOWN: &[(&str, &str)] = &[
    ("7.4", "7.4.33"),
    ("8.1", "8.1.34"),
    ("8.2", "8.2.31"),
    ("8.3", "8.3.31"),
];

/// All PHP minor versions POW knows how to install.
pub fn supported_versions() -> Vec<&'static str> {
    KNOWN.iter().map(|(v, _)| *v).collect()
}

/// Normalize any user input (`php@8.2`, `8.2`, `PHP-8.2`) to a bare minor
/// version string like `8.2`.
pub fn parse_version(input: &str) -> Result<String> {
    let cleaned = input
        .trim()
        .to_lowercase()
        .replace("php@", "")
        .replace("php-", "")
        .replace("php", "");
    let cleaned = cleaned.trim().trim_matches(|c| c == '@' || c == '-');

    // Reduce e.g. "8.2.20" -> "8.2".
    let minor: String = {
        let mut parts = cleaned.split('.');
        match (parts.next(), parts.next()) {
            (Some(major), Some(minor)) if !major.is_empty() && !minor.is_empty() => {
                format!("{major}.{minor}")
            }
            _ => bail!("could not parse a PHP version from '{input}'"),
        }
    };

    if !KNOWN.iter().any(|(v, _)| *v == minor) {
        let supported: Vec<&str> = KNOWN.iter().map(|(v, _)| *v).collect();
        bail!(
            "PHP {minor} is not supported. Supported versions: {}",
            supported.join(", ")
        );
    }
    Ok(minor)
}

/// How a given PHP version is obtained on the current platform.
pub enum Install {
    /// A ready-to-run binary archive to download + extract.
    Prebuilt(String),
    /// A source tarball to `./configure && make && make install`.
    /// `patch` is the full version so callers know the extracted dir name.
    Source { url: String, patch: String },
}

/// Decide how to install `minor` on this platform.
///
/// Prebuilt static binaries only exist for PHP 8.0+ (via static-php.dev) and
/// for Windows. PHP 7.4 on Linux/macOS has no static build anywhere, so we
/// compile it from the official source tarball instead.
pub fn strategy(minor: &str) -> Result<Install> {
    let patch = patch_for(minor)?;
    let from_source = minor.starts_with("7.") && std::env::consts::OS != "windows";
    if from_source {
        Ok(Install::Source {
            url: source_url(patch),
            patch: patch.to_string(),
        })
    } else {
        Ok(Install::Prebuilt(download_url(minor)?))
    }
}

/// Official PHP source tarball on php.net.
fn source_url(patch: &str) -> String {
    format!("https://www.php.net/distributions/php-{patch}.tar.gz")
}

/// Full patch version for a known minor version (e.g. `8.2` -> `8.2.31`).
fn patch_for(minor: &str) -> Result<&'static str> {
    KNOWN
        .iter()
        .find(|(v, _)| *v == minor)
        .map(|(_, patch)| *patch)
        .ok_or_else(|| anyhow::anyhow!("unknown PHP version {minor}"))
}

/// Resolve the download URL for a normalized minor version on the current
/// platform. Returns the URL plus a hint of the archive type for extraction.
pub fn download_url(minor: &str) -> Result<String> {
    let patch = patch_for(minor)?;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("unsupported CPU architecture: {other}"),
    };

    let url = match std::env::consts::OS {
        "linux" => format!(
            "https://dl.static-php.dev/static-php-cli/common/php-{patch}-cli-linux-{arch}.tar.gz"
        ),
        "macos" => format!(
            "https://dl.static-php.dev/static-php-cli/common/php-{patch}-cli-macos-{arch}.tar.gz"
        ),
        "windows" => windows_url(minor, patch),
        other => bail!("unsupported operating system: {other}"),
    };
    Ok(url)
}

/// Build a windows.php.net download URL. PHP 7.x is built with the `vc15`
/// toolchain and has been moved to the `archives/` folder, while 8.x uses
/// `vs16` and (for now) still lives under `releases/`.
fn windows_url(minor: &str, patch: &str) -> String {
    if minor.starts_with("7.") {
        format!(
            "https://windows.php.net/downloads/releases/archives/php-{patch}-nts-Win32-vc15-x64.zip"
        )
    } else {
        format!(
            "https://windows.php.net/downloads/releases/php-{patch}-nts-Win32-vs16-x64.zip"
        )
    }
}
