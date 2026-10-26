#!/usr/bin/env bash
# Headless screenshots of ui/ via dev/preview.html.
# usage: dev/shoot.sh OUT.png "lang=en&layout=12x2&scenario=read,sel:3" [WxH=1320x820] [light|dark=dark]
set -euo pipefail
cd "$(dirname "$0")/.."
out=$1; query=${2:-}; size=${3:-1320x820}; theme=${4:-dark}
port=$(python3 -c 'import socket;s=socket.socket();s.bind(("",0));print(s.getsockname()[1])')
python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1 &
srv=$!
profile=$(mktemp -d) # fresh browser profile: no language remembered from an earlier shot
trap 'kill $srv 2>/dev/null; rm -rf "$profile"' EXIT
sleep 0.4
scheme=$([ "$theme" = light ] && echo 1 || echo 0)
mkdir -p "$(dirname "$out")"
chromium --headless --no-sandbox --disable-gpu --hide-scrollbars --user-data-dir="$profile" \
  --blink-settings=preferredColorScheme=$scheme \
  --window-size="${size/x/,}" --virtual-time-budget=5000 \
  --screenshot="$out" "http://127.0.0.1:$port/dev/preview.html?$query" >/dev/null 2>&1
echo "$out"
