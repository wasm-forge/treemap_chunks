#!/bin/sh

set -e

cargo build --release --target wasm32-unknown-unknown --features canbench-rs

