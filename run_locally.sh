#!/bin/bash

set -e
cd ui
wasm-pack build --release --target web -- --no-default-features --features wasm
cd pkg
rm -f index.html
ln -s ../index.html .
python3 -m http.server 8000
