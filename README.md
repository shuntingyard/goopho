# goopho

Media from Google Photos? Download all to your disk!

![console demo](demo1.gif)

Tested on Linux and Windows.

## How to Use?

Currently you need to:

- Clone this from Github (`git clone --depth=1 https://github.com/shuntingyard/goopho.git`),
- make sure you have Rust/cargo available, e.g. [install](https://www.rust-lang.org/)
- and define your own Google app with scope `https://www.googleapis.com/auth/photoslibrary.readonly`.

Then on **Linux** run something like

```bash
time RUST_LOG=info cargo run -q -- -c client_secret.json --from-date 2023-10-23 mediadir
```

inside the directory you just cloned from Github.

Similarly on **Windows**:

```ps1
$Env:RUST_LOG="info"
cargo run -q -- -c client_secret.json --from-date 2023-10-23 mediadir
Remove-Item Env:\RUST_LOG
```

Or if you want to measure time elapsed in PowerShell, something like

```ps1
Measure-Command { .\target\release\goopho -c client_secret.json E:\mediadir | Out-Default }
```

will help.

(Software in this repository might easily work on other operating systems.
It just has not been tested so far.)

## Development TODOs

- [x] We don't seem to depend on Google's `get` or `batch get` API parts here.
    (According to docs this is required, however `list` seems to provide
    prefectly valid `base_url`s.)
- [x] Does `hub.media_items().list()` guarantee newest-first order?
- [x] If so, break after oldest media file selected.
- [ ] Look at distorted pics: sha1 reproducible across runs...
- [ ] Duplicate file names?
- [ ] Use [rust-binary-action](https://github.com/marketplace/actions/build-and-upload-rust-binary-to-github-releases)
    on releases?
- [ ] Test with DNS down
- [ ] Test hub module while taking link down.
- [ ] Test download module while taking link down.
- [ ] Optionally download Motion GIFs as videos?
- [x] asciinema sample?
