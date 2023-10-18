#!/bin/sh
# Trying some odd commit vs Github ... ??
RUST_LOG=debug,goopho::persistence=trace,sqlx=info cargo run -- "$@"
