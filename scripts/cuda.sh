#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export TMPDIR="${TMPDIR:-/mnt/d/tmp}"
export CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup"
export CARGO_TARGET_DIR="$HOME/cargo-target/machiavelli"
. "$HOME/.cargo/env"
if ! command -v nvcc >/dev/null; then
  echo 'BLOCKED: CUDA toolkit (nvcc) unavailable in Linux/WSL; GPU qualification not performed.' >&2
  exit 77
fi
mkdir -p "$CARGO_TARGET_DIR/native"
export MACHIAVELLI_CUDA_LIBRARY="$CARGO_TARGET_DIR/native/libmachiavelli_cuda.so"
nvcc -shared -Xcompiler=-fPIC -O3 -std=c++17 crates/machiavelli-gpu/cuda/host.cu -o "$MACHIAVELLI_CUDA_LIBRARY"
cargo test -p machiavelli-gpu --features cuda --locked --offline cuda_matches_cpu_on_hardware -- --ignored
