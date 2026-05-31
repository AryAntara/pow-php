#!/bin/sh
# POW installer — downloads a prebuilt `pow` binary for your platform, or falls
# back to building from source with cargo.
#
#   curl -fsSL https://raw.githubusercontent.com/AryAntara/pow-php/main/install.sh | sh
#
set -eu

REPO="AryAntara/pow-php"
BIN="pow"

info() { printf '\033[34m→\033[0m %s\n' "$1"; }
ok()   { printf '\033[32m✓\033[0m %s\n' "$1"; }
err()  { printf '\033[31m✗\033[0m %s\n' "$1" >&2; }

# --- Pick an install directory (prefer a no-sudo location on PATH) ----------
choose_dir() {
  for d in "$HOME/.local/bin" "/usr/local/bin"; do
    if [ -d "$d" ] && [ -w "$d" ]; then echo "$d"; return; fi
  done
  # Default: create ~/.local/bin
  echo "$HOME/.local/bin"
}

# --- Fallback: build + install from source via cargo -----------------------
install_with_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    err "No prebuilt binary available and cargo is not installed."
    err "Install Rust (https://rustup.rs) and re-run, or grab a binary from:"
    err "  https://github.com/$REPO/releases"
    exit 1
  fi
  info "Building from source with cargo (this may take a minute)…"
  cargo install --git "https://github.com/$REPO" --locked
  ok "Installed via cargo to $(dirname "$(command -v "$BIN" || echo "$HOME/.cargo/bin/$BIN")")"
  exit 0
}

# --- Detect platform -> Rust target triple ---------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)
    case "$arch" in
      x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
      *) info "No prebuilt binary for Linux/$arch."; install_with_cargo ;;
    esac ;;
  Darwin)
    case "$arch" in
      arm64|aarch64) target="aarch64-apple-darwin" ;;
      x86_64)        target="x86_64-apple-darwin" ;;
      *) info "No prebuilt binary for macOS/$arch."; install_with_cargo ;;
    esac ;;
  *)
    err "Unsupported OS '$os' for this script."
    err "On Windows, download pow-x86_64-pc-windows-msvc.zip from:"
    err "  https://github.com/$REPO/releases"
    exit 1 ;;
esac

url="https://github.com/$REPO/releases/latest/download/${BIN}-${target}.tar.gz"
dir="$(choose_dir)"
mkdir -p "$dir"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "Downloading $url"
if ! curl -fsSL "$url" -o "$tmp/$BIN.tar.gz"; then
  info "No published release found — falling back to building from source."
  install_with_cargo
fi

tar -xzf "$tmp/$BIN.tar.gz" -C "$tmp"
install -m 0755 "$tmp/$BIN" "$dir/$BIN"
ok "Installed $BIN to $dir/$BIN"

# --- PATH hint --------------------------------------------------------------
case ":$PATH:" in
  *":$dir:"*) ;;
  *) info "Add $dir to your PATH:"
     info "  export PATH=\"$dir:\$PATH\"" ;;
esac

"$dir/$BIN" --version 2>/dev/null || true
