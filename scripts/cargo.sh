#!/usr/bin/env bash
# Corre o cargo no Linux (WSL Ubuntu ou nativo) a partir da raiz do workspace.
# O target fica no sistema de ficheiros Linux: no WSL isso é o disco virtual em D:\WSL.
#   Windows:  wsl -d Ubuntu-24.04 -e bash /mnt/d/DEV/MachiavelliDB/scripts/cargo.sh test --workspace
#   Linux:    scripts/cargo.sh build --release --bin machiavelli-server
set -euo pipefail
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/cargo-target/machiavelli}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec cargo "$@"
