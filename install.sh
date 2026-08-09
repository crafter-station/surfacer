#!/bin/sh
# Install surfacer.
#
#   curl -fsSL https://raw.githubusercontent.com/crafter-station/surfacer/main/install.sh | sh
#
# Set SURFACER_VERSION to pin a release, SURFACER_INSTALL_DIR to choose where the
# binary lands.

set -eu

REPO="crafter-station/surfacer"
INSTALL_DIR="${SURFACER_INSTALL_DIR:-$HOME/.local/bin}"

err() {
	echo "install: $1" >&2
	exit 1
}

need() {
	command -v "$1" >/dev/null 2>&1 || err "missing required command: $1"
}

need uname
need tar
need mkdir

if command -v curl >/dev/null 2>&1; then
	fetch() { curl -fsSL "$1"; }
	fetch_to() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
	fetch() { wget -qO- "$1"; }
	fetch_to() { wget -qO "$2" "$1"; }
else
	err "need curl or wget"
fi

os="$(uname -s)"
arch="$(uname -m)"

case "$os/$arch" in
Darwin/arm64 | Darwin/aarch64) target="aarch64-apple-darwin" ;;
Darwin/x86_64) target="x86_64-apple-darwin" ;;
Linux/x86_64) target="x86_64-unknown-linux-gnu" ;;
*) err "unsupported platform: $os $arch" ;;
esac

version="${SURFACER_VERSION:-}"
if [ -z "$version" ]; then
	version="$(fetch "https://api.github.com/repos/$REPO/releases/latest" |
		sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
	[ -n "$version" ] || err "could not resolve the latest release; set SURFACER_VERSION"
fi

archive="surfacer-$target.tar.gz"
url="https://github.com/$REPO/releases/download/$version/$archive"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "downloading surfacer $version ($target)"
fetch_to "$url" "$tmp/$archive" || err "download failed: $url"

# Verify when the checksum is published and a checker is available; a missing
# checksum is not a reason to refuse an otherwise valid release.
if fetch_to "$url.sha256" "$tmp/$archive.sha256" 2>/dev/null; then
	if command -v shasum >/dev/null 2>&1; then
		expected="$(cut -d' ' -f1 <"$tmp/$archive.sha256")"
		actual="$(shasum -a 256 "$tmp/$archive" | cut -d' ' -f1)"
		[ "$expected" = "$actual" ] || err "checksum mismatch for $archive"
	elif command -v sha256sum >/dev/null 2>&1; then
		expected="$(cut -d' ' -f1 <"$tmp/$archive.sha256")"
		actual="$(sha256sum "$tmp/$archive" | cut -d' ' -f1)"
		[ "$expected" = "$actual" ] || err "checksum mismatch for $archive"
	fi
fi

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$INSTALL_DIR"
mv "$tmp/surfacer" "$INSTALL_DIR/surfacer"
chmod +x "$INSTALL_DIR/surfacer"

echo "installed: $INSTALL_DIR/surfacer"

case ":$PATH:" in
*":$INSTALL_DIR:"*) ;;
*)
	echo
	echo "$INSTALL_DIR is not on your PATH. Add it:"
	echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
	;;
esac
