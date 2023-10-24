# goopho

Media from Google Photos? Download all to your disk!

Tested on Linux and Windows.

## How to Use?

Currently you need to:

- Clone this from Github (`git clone --depth=1 https://github.com/shuntingyard/goopho.git`),
- [install Rust](https://www.rust-lang.org/)
- and define your own Google app with scope `https://www.googleapis.com/auth/photoslibrary.readonly`.

Then on **Linux** run someting like

```bash
time RUST_LOG=info cargo r -- -c client_secret.json --not-older 2023-10-24 mediadir
```

inside the directory you just cloned from Github.

Similarly on **Windows**:

```ps1
...
```

This might easily work on other operating systems. It just has not been tested yet.
