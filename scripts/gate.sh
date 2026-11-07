#!/bin/sh
# The gate. Stops at the first failure. Run from anywhere.
set -eu
cd "$(dirname "$0")/.."

run() {
    echo "==> $*"
    "$@"
}

run cargo fmt --all --check
run cargo clippy --workspace --all-targets --locked -- -D warnings
run cargo test --workspace --locked
echo '==> RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked'
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
run cargo check -p discord-rust --no-default-features --locked
echo "gate: all steps passed"
