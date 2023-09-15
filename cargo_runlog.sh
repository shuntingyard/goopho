#!/bin/sh
RUST_LOG=debug,sqlx=trace cargo r -- "$@"
