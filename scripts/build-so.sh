#!/usr/bin/env bash
set -euo pipefail

PROGRAM_PKG="predict-chat-program"

has_target() {
  rustc --print target-list 2>/dev/null | grep -q "^bpfel-unknown-unknown$"
}

if cargo --list | grep -q "build-sbf"; then
  echo "Detected cargo-build-sbf; building SBF artifact..."
  cargo build-sbf -p "$PROGRAM_PKG" "$@"
elif has_target; then
  echo "Building with built-in bpfel-unknown-unknown target..."
  cargo build --release --target bpfel-unknown-unknown -p "$PROGRAM_PKG" "$@"
else
  cat >&2 <<'MSG'
No Solana SBF toolchain found.
Install one of the following and re-run:
  * solana-cli (provides cargo-build-sbf) via: cargo install --locked solana-cli
  * solana-install init v1.18.18 (or matching your cluster)
  * rustup target add bpfel-unknown-unknown (if supported by your toolchain)
MSG
  exit 1
fi
