# goopho

Media from Google Photos? Download all to your disk!

Tested on Linux and Windows.

## How to Use?

Currently you need to:

- Clone this from Github (`git clone --depth=1 https://github.com/shuntingyard/goopho.git`),
- make sure you have Rust/cargo available, e.g. [install](https://www.rust-lang.org/)
- and define your own Google app with scope `https://www.googleapis.com/auth/photoslibrary.readonly`.

Then on **Linux** run someting like

```bash
time RUST_LOG=info cargo r -- -c client_secret.json --from-date 2023-10-23 mediadir
```

inside the directory you just cloned from Github.

Similarly on **Windows**:

```ps1
...
```

This might easily work on other operating systems. It just has not been tested yet.

## Development TODOs

- [ ] Add (Google) `batch get` code parts.
- [ ] Does `hub.media_items().list()` guarantee newest-first order?
- [ ] If so, break after oldest media file selected.
