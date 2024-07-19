#!/bin/bash
cargo build --release
upx --best --lzma target/release/ssgen
