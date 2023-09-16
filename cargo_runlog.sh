#!/bin/sh
RUST_LOG=debug,goopho::persistence=trace,sqlx=info cargo run -- "$@"
