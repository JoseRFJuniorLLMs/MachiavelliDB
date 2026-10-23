#!/usr/bin/env bash
set -euo pipefail
export TMPDIR=/mnt/d/tmp
export CARGO_HOME="$HOME/.cargo"
export RUSTUP_HOME="$HOME/.rustup"
export CARGO_TARGET_DIR="$HOME/cargo-target/machiavelli"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
export LIBCLANG_PATH="$CARGO_TARGET_DIR/clang"
mkdir -p "$LIBCLANG_PATH"
if [ ! -e "$LIBCLANG_PATH/libclang.so" ]; then
  source_clang=$(find /usr/lib/llvm-*/lib /lib/x86_64-linux-gnu -maxdepth 1 -name 'libclang-*.so.*' -print -quit 2>/dev/null || true)
  [ -n "$source_clang" ] || { echo "libclang not found" >&2; exit 1; }
  ln -s "$source_clang" "$LIBCLANG_PATH/libclang.so"
fi
. "$HOME/.cargo/env"
cd "$(dirname "${BASH_SOURCE[0]}")/.."
if [ "$#" -eq 0 ]; then
  exec cargo test -p machiavelli-storage --features rocksdb-backend
fi
exec cargo "$@"
