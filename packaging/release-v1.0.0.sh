#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Authorized one-time publication only. Uses the ephemeral Actions GITHUB_TOKEN.
set -euo pipefail
[[ "${GITHUB_EVENT_NAME:-}" == push && "${GITHUB_REF:-}" == refs/heads/main ]]
[[ "${GITHUB_REPOSITORY:-}" == itarqos5/spark-code ]]
python3 - <<'PY'
import tomllib
assert tomllib.load(open('Cargo.toml', 'rb'))['package']['version'] == '1.0.0'
PY
repo="$GITHUB_REPOSITORY"
assets=(
  spark-code-1.0.0-windows-x64-setup.exe
  spark-code-1.0.0-windows-x64-portable.zip
  spark-code-1.0.0-windows-x64-SHA256SUMS.txt
  spark-code-1.0.0-linux-x64.tar.gz
  spark-code-1.0.0-linux-x64-SHA256SUMS.txt
  windows-smoke.json
  linux-smoke.json
)
for asset in "${assets[@]}"; do test -s "release-assets/$asset"; done
python3 packaging/verify_release.py release-assets
cat release-assets/*-SHA256SUMS.txt
# Listing fails closed on API/auth errors instead of treating every error as not-found.
existing="$(gh api --paginate "repos/$repo/releases?per_page=100" --jq '.[] | select(.tag_name == "v1.0.0") | .id')"
if [[ -n "$existing" ]]; then
  gh release view v1.0.0 --repo "$repo" --json tagName,isDraft,assets > release-assets/existing-release.json
  python3 - <<'PY'
import json
r = json.load(open('release-assets/existing-release.json'))
assert r['tagName'] == 'v1.0.0' and not r['isDraft'], 'Existing release is incomplete; review it without replacing assets.'
required = {'spark-code-1.0.0-windows-x64-setup.exe', 'spark-code-1.0.0-windows-x64-portable.zip', 'spark-code-1.0.0-windows-x64-SHA256SUMS.txt', 'spark-code-1.0.0-linux-x64.tar.gz', 'spark-code-1.0.0-linux-x64-SHA256SUMS.txt', 'windows-smoke.json', 'linux-smoke.json'}
assert required <= {a['name'] for a in r['assets']}, 'Existing release is missing requested assets; review it without replacing assets.'
PY
  existing_dir="$(mktemp -d)"
  trap 'rm -rf -- "$existing_dir"' EXIT
  # Verify the already-published files against their own checksums, not a later build.
  gh release download v1.0.0 --repo "$repo" --dir "$existing_dir"
  python3 packaging/verify_release.py "$existing_dir"
  echo 'Verified existing v1.0.0 release; no tag, notes, or asset was changed.'
  gh release view v1.0.0 --repo "$repo" --json url,tagName,isDraft,assets
  cat "$existing_dir"/*-SHA256SUMS.txt
  exit 0
fi
# Refuse to reuse a pre-existing v1.0.0 tag pointing at some other commit.
tag_sha="$(gh api "repos/$repo/git/matching-refs/tags/v1.0.0" --jq '.[] | select(.ref == "refs/tags/v1.0.0") | .object.sha')"
if [[ -n "$tag_sha" && "$tag_sha" != "$GITHUB_SHA" ]]; then
  echo 'An existing v1.0.0 tag points elsewhere; refusing to move or reuse it.' >&2
  exit 1
fi
paths=()
for asset in "${assets[@]}"; do paths+=("release-assets/$asset"); done
gh release create v1.0.0 "${paths[@]}" --repo "$repo" --target "$GITHUB_SHA" \
  --title 'spark-code 1.0.0' --notes-file docs/RELEASE_NOTES_v1.0.0.md
# A failure here leaves the release untouched for inspection; never silently overwrite it.
gh release view v1.0.0 --repo "$repo" --json url,tagName,isDraft,assets
