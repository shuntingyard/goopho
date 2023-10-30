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
time RUST_LOG=info cargo run -- -c client_secret.json --from-date 2023-10-23 mediadir
```

inside the directory you just cloned from Github.

Similarly on **Windows**:

```ps1
$Env:RUST_LOG="info"; cargo run -- -c client_secret.json --from-date 2023-10-23 mediadir
...
```

This might easily work on other operating systems. It just has not been tested yet.

## Development TODOs

- [ ] Add (Google) `batch get` code parts. (According to docs this is required,
    However, we'll first try with base URLs retrieved with `list`.)
- [x] Does `hub.media_items().list()` guarantee newest-first order?
- [x] If so, break after oldest media file selected.
- [ ] Test with DNS down
- [ ] Test hub module while taking link down.
- [ ] Test download module while taking link down.
- [ ] Look at distorted pics. Do we happen to write more than once to some files?
