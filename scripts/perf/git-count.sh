#!/bin/bash
# The git of `pnpm perf:monorepo measure --count` (BD_GIT): records the arguments of each git process bd starts, one
# file per process in GIT_COUNT_DIR with \037 between them, then runs the real git (GIT_COUNT_REAL)
set -euo pipefail
printf '%s\037' "$@" > "$(mktemp "$GIT_COUNT_DIR/XXXXXX")"
exec "$GIT_COUNT_REAL" "$@"
