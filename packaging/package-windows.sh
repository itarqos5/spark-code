#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail
cd "$(dirname "$0")/.."
target="${SPARK_TARGET:-x86_64-pc-windows-gnu}"
output="${SPARK_OUTPUT:-dist}"
mkdir -p "$output"
cargo metadata --locked --format-version 1 --filter-platform "$target" > "$output/cargo-metadata.json"
python3 packaging/package_windows.py \
  --binary-dir "${SPARK_BINARY_DIR:-target/$target/release}" \
  --output-dir "$output" \
  --cargo-metadata "$output/cargo-metadata.json" \
  --makensis "${MAKENSIS:-makensis}" "$@"
