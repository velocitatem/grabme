#!/usr/bin/env bash
set -euo pipefail

REPO="${GRABME_REPO:-velocitatem/grabme}"
INSTALL_DIR="${GRABME_INSTALL_DIR:-$HOME/.local/bin}"
REQUESTED_VERSION="${GRABME_VERSION:-latest}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf "error: required command not found: %s\n" "$1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar
need_cmd uname

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux) target_os="unknown-linux-gnu" ;;
  *)
    printf "error: unsupported OS: %s\n" "$os" >&2
    printf "info: use 'cargo install grabme-cli' or download release manually.\n" >&2
    exit 1
    ;;
esac

case "$arch" in
  x86_64|amd64) target_arch="x86_64" ;;
  *)
    printf "error: unsupported architecture: %s\n" "$arch" >&2
    printf "info: use 'cargo install grabme-cli' or build from source.\n" >&2
    exit 1
    ;;
esac

if [[ "$REQUESTED_VERSION" == "latest" ]]; then
  api_url="https://api.github.com/repos/${REPO}/releases/latest"
  version="$(curl -fsSL "$api_url" | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p' | head -n1)"
  if [[ -z "$version" ]]; then
    printf "error: failed to resolve latest version from GitHub API\n" >&2
    exit 1
  fi
else
  version="${REQUESTED_VERSION#v}"
fi

target="${target_arch}-${target_os}"
archive="grabme-${version}-${target}.tar.gz"
base_url="https://github.com/${REPO}/releases/download/v${version}"
archive_url="${base_url}/${archive}"
checksum_url="${archive_url}.sha256"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

printf "Installing grabme v%s for %s\n" "$version" "$target"
printf "Downloading %s\n" "$archive_url"
curl -fL "$archive_url" -o "$tmpdir/$archive"

printf "Downloading checksum\n"
curl -fL "$checksum_url" -o "$tmpdir/$archive.sha256"

printf "Verifying checksum\n"
(cd "$tmpdir" && sha256sum -c "$archive.sha256")

mkdir -p "$INSTALL_DIR"
tar -xzf "$tmpdir/$archive" -C "$tmpdir"
install -m 0755 "$tmpdir/grabme" "$INSTALL_DIR/grabme"

printf "\nDone. Installed to: %s/grabme\n" "$INSTALL_DIR"
printf "Run: %s/grabme --help\n" "$INSTALL_DIR"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    printf "\nNote: %s is not on your PATH.\n" "$INSTALL_DIR"
    printf "Add this to your shell profile:\n"
    printf "  export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
    ;;
esac
