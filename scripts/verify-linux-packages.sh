#!/usr/bin/env bash
set -euo pipefail

install=${4:-}
if [[ "$install" != "" && "$install" != "--install" ]]; then
  echo 'Unsupported verification option' >&2; exit 1
fi
if [[ "$install" == "--install" ]]; then
  if [[ "${GITHUB_ACTIONS:-}" != "true" || "${RUNNER_OS:-}" != "Linux" || "${RUNNER_ENVIRONMENT:-}" != "github-hosted" ]]; then
    echo 'LINUX_PACKAGE_INSTALL_CI_ONLY' >&2; exit 1
  fi
  source /etc/os-release
  if [[ "$ID" != "ubuntu" || "$VERSION_ID" != "24.04" ]]; then
    echo 'LINUX_PACKAGE_INSTALL_REQUIRES_UBUNTU_24_04' >&2; exit 1
  fi
fi

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
node scripts/verify-linux-deb.mjs "$deb" "${tag#v}" "$deb_arch"
test -s "$image.sig"
test -s "$deb.sig"

root=$(mktemp -d)
package=""
installed=false
cleanup() {
  if [[ "$installed" == true ]]; then sudo dpkg --purge "$package"; fi
  rm -rf "$root"
}
trap cleanup EXIT
dpkg-deb --extract "$deb" "$root/deb"
node scripts/verify-desktop-bundle.mjs "$root/deb"
mkdir "$root/image"
(cd "$root/image" && "$image" --appimage-extract >/dev/null)
node scripts/verify-desktop-bundle.mjs "$root/image/squashfs-root"

# Keep the smoke test's application data separate from the runner's home.
mkdir -p "$root/home" "$root/config" "$root/data" "$root/cache" "$root/runtime"
chmod 700 "$root/runtime"
smoke() {
  env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" XDG_CACHE_HOME="$root/cache" XDG_RUNTIME_DIR="$root/runtime" WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  xvfb-run -a dbus-run-session -- bash -s -- "$1" "$root/startup.log" <<'SMOKE'
set -euo pipefail
"$1" >"$2" 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT
for attempt in $(seq 1 30); do
  if ! kill -0 "$pid" 2>/dev/null; then
    cat "$2" >&2
    echo 'Linux desktop process exited before opening its window.' >&2
    exit 1
  fi
  if xdotool search --onlyvisible --name '^Skills Hub$' >/dev/null 2>&1; then
    if grep -Fq 'Could not register the Skills Hub collection link protocol.' "$2"; then
      cat "$2" >&2
      exit 1
    fi
    echo 'Linux desktop window opened successfully.'
    exit 0
  fi
  sleep 1
done
echo 'Linux desktop window did not open.' >&2
exit 1
SMOKE
}
smoke "$root/image/squashfs-root/AppRun"

if [[ "$install" == "--install" ]]; then
  package=$(dpkg-deb -f "$deb" Package)
  if dpkg-query -W -f='${Status}' "$package" 2>/dev/null | grep -q 'install ok installed'; then
    echo 'LINUX_PACKAGE_INSTALL_REQUIRES_CLEAN_RUNNER' >&2; exit 1
  fi
  # Use the real payload with a synthetic predecessor version to exercise dpkg upgrade.
  dpkg-deb --raw-extract "$deb" "$root/predecessor"
  sed -i 's/^Version:.*/Version: 0.0.0~verification/' "$root/predecessor/DEBIAN/control"
  dpkg-deb --build "$root/predecessor" "$root/predecessor.deb" >/dev/null
  installed=true
  sudo apt-get install --no-install-recommends -y "$root/predecessor.deb"
  test "$(dpkg-query -W -f='${Version}' "$package")" = '0.0.0~verification'
  sudo apt-get install --no-install-recommends -y "$deb"
  test "$(dpkg-query -W -f='${Status}' "$package")" = 'install ok installed'
  test "$(dpkg-query -W -f='${Version}' "$package")" = "${tag#v}"
  test "$(dpkg-query -W -f='${Architecture}' "$package")" = "$deb_arch"
  executable=$(find "$root/deb/usr/bin" -maxdepth 1 -type f -executable)
  test "$(printf '%s\n' "$executable" | wc -l)" -eq 1
  executable="/usr/bin/$(basename "$executable")"
  test -x "$executable"
  dpkg-query -S "$executable" | grep -F "$package: $executable"
  smoke "$executable"
  echo 'Debian install, version upgrade and installed desktop startup verified.'
fi
