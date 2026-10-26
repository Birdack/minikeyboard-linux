#!/bin/sh
# Build an Arch Linux package from this checkout and install it with pacman.
#   packaging/arch/install.sh            build and install (pacman asks for sudo)
#   packaging/arch/install.sh --build    only build the .pkg.tar.zst
#   packaging/arch/install.sh --remove   uninstall (sudo pacman -Rns minikeyboard)
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
SRC=$(cd "$HERE/../.." && pwd)

if [ "${1:-}" = "--remove" ]; then
    exec sudo pacman -Rns minikeyboard
fi

if ! command -v makepkg >/dev/null 2>&1; then
    echo "makepkg not found: this script is for Arch Linux and derivatives (Manjaro, EndeavourOS...)."
    exit 1
fi

PKGVER=$(sed -n 's/^pkgver=//p' "$HERE/PKGBUILD")
BUILD=$(mktemp -d)
trap 'rm -rf "$BUILD"' EXIT

# makepkg uses a local file named like the source instead of downloading it,
# so the package is built from this checkout (uncommitted changes to tracked files included).
TREE=$(git -C "$SRC" stash create 2>/dev/null || true)
git -C "$SRC" archive --format=tar.gz --prefix="minikeyboard-linux-$PKGVER/" \
    -o "$BUILD/minikeyboard-linux-$PKGVER.tar.gz" "${TREE:-HEAD}"
cp "$HERE/PKGBUILD" "$HERE/minikeyboard.install" "$BUILD/"

echo "Building the package (takes a few minutes the first time)..."
if [ "${1:-}" = "--build" ]; then
    (cd "$BUILD" && makepkg --syncdeps --skipchecksums --needed)
    cp "$BUILD"/minikeyboard-*.pkg.tar.zst "$HERE/"
    echo "Package written to $HERE/"
else
    (cd "$BUILD" && makepkg --syncdeps --skipchecksums --install --needed)
    echo "Installed. Start \"MINI KEYBOARD\" from the application menu or run: minikeyboard"
fi
