#!/usr/bin/env bash
set -euo pipefail

for suite in cli_bridge cli_terminal services::tests::agent_access; do
  cargo test --locked --manifest-path src-tauri/Cargo.toml --lib --features cli "$suite"
done
