#!/bin/sh
# Renders the icons from art/icons/make.py. Needs python3, the rive CLI and ffmpeg.
#   app/src-tauri/icons/tray.rgba   44x44 raw RGBA menu-bar template (black + alpha), loaded by lib.rs
#   art/icons/build/app-icon.png    1024x1024 app icon; `npx tauri icon` turns it into app/src-tauri/icons/*
set -eu
cd "$(dirname "$0")"
python3 make.py
mkdir -p build
for p in tray app app-mask; do (cd "$p" && rive . --screenshot="../build/$p.png" --advance=1 >/dev/null 2>&1); done
# rive writes an opaque #1d1d1d background: brightness (the red channel; the masks are white) above it becomes alpha
alpha="geq=r=0:g=0:b=0:a='clip((r(X,Y)-29)*255/226,0,255)'"
ffmpeg -y -loglevel error -i build/tray.png -vf "format=rgba,$alpha,scale=44:44:flags=lanczos" -f rawvideo -pix_fmt rgba ../../app/src-tauri/icons/tray.rgba
ffmpeg -y -loglevel error -i build/tray.png -vf "format=rgba,$alpha,scale=44:44:flags=lanczos" build/tray-preview.png
ffmpeg -y -loglevel error -i build/app.png -i build/app-mask.png -filter_complex "[1]format=gray,geq=lum='clip((lum(X,Y)-29)*255/226,0,255)'[m];[0][m]alphamerge" build/app-icon.png
ffmpeg -y -loglevel error -f lavfi -i color=white:s=44x44 -i build/tray-preview.png -filter_complex "[0][1]overlay,scale=176:176:flags=neighbor" -frames:v 1 build/tray-on-white.png
echo "wrote app/src-tauri/icons/tray.rgba and art/icons/build/app-icon.png"
