#!/usr/bin/env bash
set -euo pipefail

# Navigate to the root of the repository.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Export the Cargo bin directory to the PATH.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

# Check for required tools: cargo-xwin and cbindgen.
if ! cargo xwin --version >/dev/null 2>&1; then
    printf '%s\n' 'Missing cargo-xwin (install with: cargo install cargo-xwin --locked)' >&2
    exit 1
fi
if ! command -v cbindgen >/dev/null 2>&1; then
    printf '%s\n' 'Missing cbindgen (install with: cargo install cbindgen --locked)' >&2
    exit 1
fi

# Set up the target and output directories.
target_dir="${CARGO_TARGET_DIR:-target}"
output_dir="$target_dir/neteq-c"

# Build NetEQ for Linux and Windows targets.
cargo build --release -p neteq --lib --features native --target x86_64-unknown-linux-gnu
cargo xwin build --release -p neteq --lib --features native --target x86_64-pc-windows-msvc

# Generate the C header using cbindgen.
mkdir -p "$output_dir"
cbindgen --config neteq/cbindgen.toml --crate neteq --output "$output_dir/neteq.h"

# Copy the built libraries to the output directories.
mkdir -p "$output_dir/linux" "$output_dir/windows"
cp "$target_dir/x86_64-unknown-linux-gnu/release/libneteq.so" "$output_dir/linux/"
cp "$target_dir/x86_64-pc-windows-msvc/release/neteq.dll" "$output_dir/windows/"
cp "$target_dir/x86_64-pc-windows-msvc/release/neteq.dll.lib" "$output_dir/windows/neteq.lib"

printf 'NetEQ C libraries and header: %s\n' "$output_dir"
