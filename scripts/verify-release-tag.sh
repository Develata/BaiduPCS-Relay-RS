#!/usr/bin/env bash
set -euo pipefail

tag="${1:-}"
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    printf 'error: release tag must match vMAJOR.MINOR.PATCH, got %q\n' "$tag" >&2
    exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

package_version="$({ cargo metadata --locked --no-deps --format-version 1; } | python3 -c '
import json
import os
import sys

metadata = json.load(sys.stdin)
manifest = os.path.realpath("Cargo.toml")
for package in metadata["packages"]:
    if os.path.realpath(package["manifest_path"]) == manifest:
        print(package["version"])
        break
else:
    raise SystemExit("root Cargo package not found")
')"

tag_version="${tag#v}"
if [[ "$tag_version" != "$package_version" ]]; then
    printf 'error: tag version %s does not match Cargo.toml version %s\n' \
        "$tag_version" "$package_version" >&2
    exit 1
fi

printf 'release tag %s matches Cargo.toml version %s\n' "$tag" "$package_version"
