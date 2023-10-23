# goopho

Google Photos? Get them down to your disk.

Tested on Linux and Windows

## Operation

Currently you need to:

- Clone it from Github (`git clone --depth=1 https://github.com/shuntingyard/goopho.git`),
- [install Rust(https://www.rust-lang.org/)] and build a binary
- and define your own Google app with scope `https://www.googleapis.com/auth/photoslibrary.readonly`.

Then do someting like

```bash
time RUST_LOG=info cargo r
```

on Linux or

```ps1
...
```

on Windows.
