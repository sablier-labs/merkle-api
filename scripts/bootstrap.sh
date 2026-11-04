#!/bin/sh
set -eu

cd "$(dirname "$0")"
: "${DOTENV_PRIVATE_KEY_PRODUCTION:?DOTENV_PRIVATE_KEY_PRODUCTION must be set}"

exec ./dotenvx run --quiet --strict --overload \
  --no-armor --no-native --no-1password --no-bitwarden \
  -f .env.production -- ./merkle-api "$@"
