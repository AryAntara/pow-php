//! User-facing logging helpers plus a couple of shared system helpers
//! (download/extract, process control). All output to the user must go through
//! here so the look-and-feel stays consistent across the whole tool.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Green check — an operation completed.
pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

/// Blue arrow — neutral information.
pub fn info(msg: &str) {
    println!("{} {}", "→".blue().bold(), msg);
}

/// Yellow bang — something the user should be aware of, but not fatal.
pub fn warn(msg: &str) {
    println!("{} {}", "!".yellow().bold(), msg.yellow());
}

/// Red cross — an error. Printed to stderr.
pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg.red());
}

/// Download `url` into `dest_dir`, then extract it in place. The archive type
/// (`.zip` / `.tar.gz`) is inferred from the URL. Extraction is delegated to
/// the system `tar`, which ships on every modern Windows/macOS/Linux and
/// handles both formats — that keeps the binary small (no zip/tar crates).
pub async fn download_and_extract(url: &str, dest_dir: &Path) -> Result<()> {
    fs::create_dir_all(dest_dir)
        .with_context(|| format!("failed to create {}", dest_dir.display()))?;

    let file_name = url.rsplit('/').next().unwrap_or("download.archive");
    let archive_path = dest_dir.join(file_name);

    info(&format!("Downloading {url}"));
    let resp = reqwest::get(url)
        .await
        .with_context(|| format!("request to {url} failed"))?;
    if !resp.status().is_success() {
        bail!("download failed: HTTP {} for {url}", resp.status());
    }

    let total = resp.content_length().unwrap_or(0);
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-"),
    );

    let mut file = fs::File::create(&archive_path)
        .with_context(|| format!("failed to create {}", archive_path.display()))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("error while streaming download")?;
        std::io::Write::write_all(&mut file, &chunk).context("failed writing archive to disk")?;
        bar.inc(chunk.len() as u64);
    }
    bar.finish_and_clear();
    drop(file);

    info("Extracting…");
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(dest_dir)
        .status()
        .context("failed to run `tar` (is it installed and on PATH?)")?;
    if !status.success() {
        bail!("extraction failed for {}", archive_path.display());
    }

    // Tidy up the archive once extracted.
    let _ = fs::remove_file(&archive_path);
    Ok(())
}

/// Kill a process by PID in a cross-platform way.
pub fn kill_pid(pid: u32) -> Result<()> {
    let status = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
    } else {
        Command::new("kill").arg(pid.to_string()).status()
    }
    .context("failed to spawn the kill command")?;

    if !status.success() {
        bail!("could not stop process {pid} (already gone?)");
    }
    Ok(())
}

/// Write a PID to a pid-file.
pub fn write_pid(path: &Path, pid: u32) -> Result<()> {
    fs::write(path, pid.to_string())
        .with_context(|| format!("failed to write pid file {}", path.display()))?;
    Ok(())
}

/// Read a PID from a pid-file, if it exists and parses.
pub fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Remove a pid-file, ignoring a missing file.
pub fn clear_pid(path: &Path) {
    let _ = fs::remove_file(path);
}

/// Best-effort check for whether a PID is currently alive.
pub fn is_running(pid: u32) -> bool {
    if cfg!(windows) {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    } else {
        // Signal 0 just checks for existence/permission.
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
