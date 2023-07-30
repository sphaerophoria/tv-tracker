#!/usr/bin/env bash

set -ex

LINT_DIR=$(dirname $0)
prettier -c $(find "$LINT_DIR"/res -iname "*.html" -o -iname "*.js")

cargo fmt --check
cargo clippy -- -D warnings
cargo test
