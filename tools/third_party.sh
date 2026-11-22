#!/bin/sh
# Regenerate apps/atari_editor/THIRD_PARTY.txt: the Rust libraries built
# into the editor (on any platform) and their licences. Run after changing
# dependencies.
set -e
cd "$(dirname "$0")/.."
out=apps/atari_editor/THIRD_PARTY.txt
{
    echo "SDL2 (the C library, built in by the sdl2 crate) - Zlib"
    echo "  Copyright (C) 1997-2025 Sam Lantinga"
    echo
    echo "Rust libraries:"
    cargo tree -p atari_editor -e normal --prefix none --format "{p}|{l}" --target all |
        grep -v '(/' |
        sed -e 's/ (\*)$//' -e 's/^\([^ ]*\) v\([^|]*\)|\(.*\)$/  \1 \2 - \3/' |
        sort -u
} > "$out"
echo "wrote $out"
