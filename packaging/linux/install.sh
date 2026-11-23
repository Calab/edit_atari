#!/bin/sh
# Install Edit Atari for the current user: the program in ~/.local/bin,
# its icon, and a menu entry. Run from the unpacked folder.
set -e
cd "$(dirname "$0")"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$HOME/.local/bin" "$data/applications"
install -m 755 atari_editor "$HOME/.local/bin/atari_editor"
# Atari fonts for Options > Fonts.
mkdir -p "$data/atari_editor/fonts"
cp fonts/FNT/* "$data/atari_editor/fonts/"
for size in 16 32 48 64 128 256 512; do
    dir="$data/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$dir"
    install -m 644 "icons/atari_editor-$size.png" "$dir/atari_editor.png"
done
sed "s|^Exec=atari_editor|Exec=$HOME/.local/bin/atari_editor|" atari_editor.desktop \
    > "$data/applications/atari_editor.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$data/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$data/icons/hicolor" 2>/dev/null || true
echo "Installed. Edit Atari is in your applications menu;"
echo "the program is $HOME/.local/bin/atari_editor."
