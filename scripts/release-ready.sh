#!/usr/bin/env bash
set -euo pipefail

printf "[0/6] metadata sanity\n"
if ! rg -n "^repository\\s*=" Cargo.toml >/dev/null 2>&1; then
  printf "warning: Cargo.toml missing 'repository' field\n"
fi
if ! rg -n "^homepage\\s*=" Cargo.toml >/dev/null 2>&1; then
  printf "warning: Cargo.toml missing 'homepage' field\n"
fi

printf "[1/6] rustfmt check\n"
cargo fmt --all -- --check

printf "[2/6] clippy\n"
cargo clippy --all-targets --all-features -- -D warnings

printf "[3/6] tests\n"
cargo test --all-targets --all-features

printf "[4/6] package verification\n"
cargo package --allow-dirty

printf "[5/6] publish dry-run\n"
cargo publish --dry-run --allow-dirty

printf "[6/6] renderer syntax check\n"
node --check renderer-runtime/render.mjs

printf "Release preflight passed.\n"
