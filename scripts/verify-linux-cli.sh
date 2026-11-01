#!/usr/bin/env bash
set -euo pipefail
binary=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
node scripts/verify-linux-cli.mjs "$binary"
docker run --rm --mount "type=bind,src=$binary,dst=/opt/skillshub-cli,readonly" ubuntu:24.04 sh -c '
  set -eu
  apt-get update -qq
  apt-get install --no-install-recommends -y libdbus-1-3 zlib1g >/dev/null
  /opt/skillshub-cli version --json
  /opt/skillshub-cli doctor --json
'
