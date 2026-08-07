#!/usr/bin/env bash
# Build & unit tests. Only check: cargo build / cargo test.

test_build() {
  [ -n "${NO_BUILD:-}" ] && { echo "  skip: --no-build"; exit 2; }
  cargo build $BUILD_FLAG --quiet || { echo "  FAIL: cargo build"; exit 1; }
}

test_unit_tests() {
  [ -n "${NO_BUILD:-}" ] && { echo "  skip: --no-build"; exit 2; }
  cargo test --quiet || { echo "  FAIL: cargo test"; exit 1; }
}
