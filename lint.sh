#!/usr/bin/env bash

set -ex

LINT_DIR=$(dirname $0)
prettier -c $(find "$LINT_DIR"/res -iname "*.html" -o -iname "*.js")
eslint $(find "$LINT_DIR"/res -iname "*.js")

PYTHON_FILES=$(git ls-files | grep "\.py$")
black --check $PYTHON_FILES
ruff $PYTHON_FILES

cargo fmt --check
cargo clippy -- -D warnings
cargo test
