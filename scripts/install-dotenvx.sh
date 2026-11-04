#!/usr/bin/env bash
set -euo pipefail

destination="${1:?Usage: bash scripts/install-dotenvx.sh <destination>}"
archive="$(mktemp)"
trap 'rm -f "$archive"' EXIT

curl --fail --silent --show-error --location \
  https://github.com/dotenvx/dotenvx/releases/download/v2.38.2/dotenvx-2.38.2-linux-amd64.tar.gz \
  --output "$archive"
printf '%s  %s\n' '37d8e26600b707e4ec450b1ab57571159f6ac39be736b04886a0fcc66527a1bb' "$archive" | sha256sum --check --status
mkdir -p "$destination"
tar -xzf "$archive" -C "$destination" dotenvx
chmod 755 "$destination/dotenvx"
