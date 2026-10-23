#!/usr/bin/env bash
set -euo pipefail
export TMPDIR=/mnt/d/tmp
export ELAN_HOME="${ELAN_HOME:-$HOME/.elan}"
export XDG_CACHE_HOME=/mnt/d/tmp/lean-cache
export PATH="$ELAN_HOME/bin:$PATH"
cd "$(dirname "${BASH_SOURCE[0]}")/../lean"
exec lake "$@"
