#!/bin/sh
# Build and install MINI KEYBOARD for the current user (no system files except the udev rule).
#   ./install.sh            build and install (asks before using sudo for the udev rule)
#   ./install.sh --remove   uninstall
set -eu

SRC=$(cd "$(dirname "$0")" && pwd)
DATA=${XDG_DATA_HOME:-$HOME/.local/share}
BIN=$HOME/.local/bin/minikeyboard
DESKTOP=$DATA/applications/minikeyboard.desktop
ICON=$DATA/icons/hicolor/scalable/apps/minikeyboard.svg
RULE=/etc/udev/rules.d/70-minikeyboard.rules

if [ "${1:-}" = "--remove" ]; then
    rm -f "$BIN" "$DESKTOP" "$ICON"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA/applications" 2>/dev/null || true
    echo "Program removed."
    if [ -e "$RULE" ]; then
        echo "To remove the udev rule too: sudo rm $RULE && sudo udevadm control --reload"
    fi
    exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust (cargo) is missing. Install it from https://rustup.rs or:"
    echo "  Arch:          sudo pacman -S rust"
    echo "  Debian/Ubuntu: sudo apt install cargo"
    echo "  Fedora:        sudo dnf install cargo"
    exit 1
fi

if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
    echo "The WebKitGTK 4.1 development files (and pkg-config) are missing. Install for example:"
    echo "  Arch:          sudo pacman -S --needed base-devel pkgconf webkit2gtk-4.1"
    echo "  Debian/Ubuntu: sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libssl-dev"
    echo "  Fedora:        sudo dnf install gcc pkgconf-pkg-config webkit2gtk4.1-devel gtk3-devel libsoup3-devel openssl-devel"
    exit 1
fi

echo "Building (takes a few minutes the first time)..."
(cd "$SRC/src-tauri" && cargo build --release)

mkdir -p "$(dirname "$BIN")" "$(dirname "$DESKTOP")" "$(dirname "$ICON")"
install -m 755 "$SRC/src-tauri/target/release/minikeyboard" "$BIN"
sed "s|^Exec=.*|Exec=$BIN|" "$SRC/data/minikeyboard.desktop" > "$DESKTOP"
install -m 644 "$SRC/data/icon.svg" "$ICON"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true

echo "Installed: $BIN (and in the application menu as \"MINI KEYBOARD\")."
case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) echo "Note: $HOME/.local/bin is not in PATH; the menu entry works anyway." ;;
esac

if [ -e "$RULE" ] && cmp -s "$RULE" "$SRC/data/70-minikeyboard.rules"; then
    echo "udev rule already installed."
else
    echo
    echo "A udev rule is needed to use the keyboard without root:"
    echo "  sudo install -m 644 $SRC/data/70-minikeyboard.rules $RULE"
    echo "  sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw"
    printf "Install it now? [y/N] "
    read -r answer || answer=
    case "$answer" in
        s|S|si|sì|Si|SI|y|Y)
            sudo install -m 644 "$SRC/data/70-minikeyboard.rules" "$RULE"
            sudo udevadm control --reload
            sudo udevadm trigger --subsystem-match=hidraw
            echo "Done. If the keyboard was already plugged in, unplug and replug it." ;;
        *) echo "Skipped: you can install it later with the commands above." ;;
    esac
fi
