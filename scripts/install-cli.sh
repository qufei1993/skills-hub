#!/usr/bin/env bash
# Install the standalone release binary, independently of the desktop bridge.
set -euo pipefail
work=''
stage=''

main() {
  local os arch platform release_url tag version asset base dest bin_dir
  os=$(uname -s); arch=$(uname -m)
  case "$os" in Darwin) platform=darwin ;; Linux) platform=linux ;; *) echo "Unsupported system: $os" >&2; return 1 ;; esac
  case "$arch" in arm64|aarch64) platform+=-arm64 ;; x86_64|amd64) platform+=-x64 ;; *) echo "Unsupported architecture: $arch" >&2; return 1 ;; esac
  command -v curl >/dev/null || { echo 'curl is required.' >&2; return 1; }
  if ! command -v shasum >/dev/null && ! command -v sha256sum >/dev/null; then
    echo 'shasum or sha256sum is required.' >&2; return 1
  fi
  bin_dir="$HOME/.local/bin"
  dest="$bin_dir/skillshub-cli"
  if [[ -L "$dest" || ( -e "$dest" && ! -f "$dest" ) ]]; then
    echo "Refusing to replace a symlink or non-file: $dest" >&2; return 1
  fi
  echo 'Finding the latest Skills Hub release...'
  release_url=$(curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL --retry 2 --connect-timeout 15 --max-time 120 -o /dev/null -w '%{url_effective}' 'https://github.com/qufei1993/skills-hub/releases/latest')
  tag=${release_url#https://github.com/qufei1993/skills-hub/releases/tag/}
  if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo 'Could not resolve a stable release version.' >&2; return 1
  fi
  version=${tag#v}
  asset="skillshub-cli-$version-$platform"
  base="https://github.com/qufei1993/skills-hub/releases/download/$tag"
  work=$(mktemp -d "${TMPDIR:-/tmp}/skillshub-install.XXXXXX")
  trap 'rm -rf "$work"; if [[ -n "$stage" ]]; then rm -f "$stage"; fi' EXIT
  echo "Downloading $asset..."
  if ! curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL --retry 2 --connect-timeout 15 --max-time 300 "$base/$asset" -o "$work/$asset" ||
     ! curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL --retry 2 --connect-timeout 15 --max-time 120 "$base/$asset.sha256" -o "$work/checksum"; then
    echo "Download failed. This release may not contain a CLI for $platform (CLI assets require v0.11.0 or later)." >&2
    return 1
  fi
  local expected name extra actual
  read -r expected name extra < "$work/checksum"
  if [[ ! "$expected" =~ ^[[:xdigit:]]{64}$ || "$name" != "$asset" || -n "$extra" || $(wc -l < "$work/checksum") -ne 1 ]]; then
    echo 'Invalid SHA-256 file.' >&2; return 1
  fi
  if command -v sha256sum >/dev/null; then
    actual=$(sha256sum "$work/$asset")
  else
    actual=$(shasum -a 256 "$work/$asset")
  fi
  actual=${actual%% *}
  if [[ "$actual" != "$expected" ]]; then echo 'SHA-256 mismatch; existing CLI was not changed.' >&2; return 1; fi
  mkdir -p "$bin_dir"
  stage=$(mktemp "$bin_dir/.skillshub-cli.XXXXXX")
  cp "$work/$asset" "$stage"
  chmod 755 "$stage"
  mv -f "$stage" "$dest"
  stage=''
  echo "Installed Skills Hub CLI $version: $dest"
  configure_path "$bin_dir"
  echo "Verify with: \"$dest\" version --json"
  rm -rf "$work"
  trap - EXIT
}

configure_path() {
  local bin_dir=$1 profile escaped line
  local profiles=()
  # Quote literal paths so spaces, quotes and shell metacharacters remain data.
  escaped=$(printf '%s' "$bin_dir" | sed "s/'/'\\\\''/g")
  line="export PATH='$escaped':\"\$PATH\""
  case "${SHELL:-}" in
    */zsh|zsh) profiles=("${ZDOTDIR:-$HOME}/.zshrc") ;;
    */bash|bash)
      profiles=("$HOME/.bashrc")
      if [[ -f "$HOME/.bash_profile" ]]; then profiles+=("$HOME/.bash_profile")
      elif [[ -f "$HOME/.bash_login" ]]; then profiles+=("$HOME/.bash_login")
      else profiles+=("$HOME/.profile"); fi ;;
    *) echo "Add $bin_dir to your shell PATH to use skillshub-cli by name."; return ;;
  esac
  for profile in "${profiles[@]}"; do
    if ! grep -Fqx "$line" "$profile" 2>/dev/null; then
      if ! printf '\n# Skills Hub standalone CLI\n%s\n' "$line" >> "$profile"; then
        echo "Could not update $profile. Add $bin_dir to PATH manually." >&2
      fi
    fi
  done
  echo 'Open a new terminal to use skillshub-cli. Run this installer again to upgrade.'
}

# Keep the invocation last so a truncated piped download cannot start installation.
main "$@"
