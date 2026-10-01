#!/usr/bin/env bash
set -euo pipefail

assets=$(cd "$1" && pwd)
tag=$2
target=$3
case "$target" in
  x86_64-unknown-linux-gnu) arch=x64; deb_arch=amd64 ;;
  aarch64-unknown-linux-gnu) arch=arm64; deb_arch=arm64 ;;
  *) echo 'Unsupported Linux target' >&2; exit 1 ;;
esac
test "$(rustc -vV | sed -n 's/^host: //p')" = "$target"
deb="$assets/Skills-Hub-$tag-Linux-$arch.deb"
image="$assets/Skills-Hub-$tag-Linux-$arch.AppImage"
test "$(dpkg-deb -f "$deb" Architecture)" = "$deb_arch"
test "$(dpkg-deb -f "$deb" Version)" = "${tag#v}"
test -s "$image.sig"

root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT
dpkg-deb --extract "$deb" "$root/deb"
node scripts/verify-desktop-bundle.mjs "$root/deb"
mkdir "$root/image"
(cd "$root/image" && "$image" --appimage-extract >/dev/null)
node scripts/verify-desktop-bundle.mjs "$root/image/squashfs-root"

# Keep the smoke test's application data separate from the runner's home.
mkdir -p "$root/home" "$root/config" "$root/data" "$root/cache" "$root/runtime"
chmod 700 "$root/runtime"
env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" XDG_CACHE_HOME="$root/cache" XDG_RUNTIME_DIR="$root/runtime" WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  xvfb-run -a bash -s -- "$root/image/squashfs-root/AppRun" <<'SMOKE'
set -euo pipefail
"$1" >/dev/null 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT
for attempt in $(seq 1 30); do
  kill -0 "$pid"
  if xdotool search --onlyvisible --name '^Skills Hub$' >/dev/null 2>&1; then
    echo 'Linux desktop window opened successfully.'
    exit 0
  fi
  sleep 1
done
echo 'Linux desktop window did not open.' >&2
exit 1
SMOKE
