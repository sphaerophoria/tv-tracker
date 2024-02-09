#!/usr/bin/env bash

set -ex

LINT_DIR=$(dirname $0)
prettier -c $(find "$LINT_DIR"/res -iname "*.html" -o -iname "*.js")
eslint $(find "$LINT_DIR"/res -iname "*.js")

PYTHON_FILES=$(git ls-files | grep "\.py$")
black --check $PYTHON_FILES
ruff $PYTHON_FILES

ALL_FILES=$(git ls-files | grep -v ^lint.sh$)
if grep -q FIXME $ALL_FILES; then
	echo "FIXMEs remain"
	exit 1
fi

cargo fmt --check
cargo clippy -- -D warnings
cargo test

