#!/bin/sh

set -ex

# rustfmt src/lib.rs
wasm-pack build --release --target web
python3 -m http.server