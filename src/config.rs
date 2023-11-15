//! Abstraction over configuration details

use std::{ffi::OsString, path::PathBuf};

use anyhow::{bail, Context};
use argh::FromArgs;
use directories::BaseDirs;
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    chrono::NaiveDate,
    oauth2::{ApplicationSecret, ConsoleApplicationSecret},
};
use tokio::fs;

/// This app's command line args
#[derive(FromArgs, Debug)]
/// Download images and videos from Google Photos
pub struct Cmdlargs {
    /// just show what would be written
    #[argh(switch, short = 'd')]
    pub dry_run: bool,

    /* Not sure if we ever want to implement this?
     *
    /// persist creation date from Google Photos to disk
    #[argh(switch, long = "pcd")]
    pub persist_creation_date: bool,
     */
    /// don't select media files created earlier (year-month-day)
    #[argh(option, short = 'f')]
    pub from_date: Option<NaiveDate>,

    /// don't select media files created later
    #[argh(option, short = 't')]
    pub to_date: Option<NaiveDate>,

    /// path to client secret file (the one you got from Google)
    #[argh(option, short = 'c')]
    pub client_secret: PathBuf,

    /// target folder (must *not* exist)
    #[argh(positional)]
    pub target: PathBuf,

    /// don't force processing to be FIFO
    #[argh(switch, short = 'u')]
    pub unordered: bool,
}

/// Provide `OsString` to a file inside user's local data directory
pub fn get_token_store_path() -> anyhow::Result<OsString> {
    let mut _buf;
    let store;
    if let Some(base_dirs) = BaseDirs::new() {
        let home_data = base_dirs.data_local_dir();
        _buf = home_data.to_owned();
        _buf.push("goopho-tokens.json");
        store = _buf.into_os_string()
    } else {
        bail!("Something is bad with your home directory");
    }

    Ok(store)
}

/// Extract application secret from Google's `client_secret.json` file
pub async fn get_app_secret(path: PathBuf) -> anyhow::Result<ApplicationSecret> {
    let client_secret = fs::read_to_string(path).await?;
    let secret: ConsoleApplicationSecret = serde_json::from_str(&client_secret)?;
    let app_secret = secret
        .installed
        .context("Wrong client secret format, expected `installed`")?;

    Ok(app_secret)
}
