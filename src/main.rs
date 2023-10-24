use std::path::PathBuf;

use anyhow::Context;
use argh::FromArgs;
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    api::{ListMediaItemsResponse, MediaMetadata},
    chrono::NaiveDate,
    hyper, hyper_rustls, oauth2, Error, PhotosLibrary,
};
use tokio::fs;
use tracing::{debug, error, info};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Subscribe to traces.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()) // Read trace levels from RUST_LOG env var.
        .init();

    // Command line args used
    #[derive(FromArgs, Debug)]
    /// get images and videos from Google Photos
    struct Config {
        /// just show what would be written
        #[argh(switch, short = 'd')]
        _dry_run: bool,

        /// limit age of files to download by year-month-day
        #[argh(option, short = 'l')]
        not_older: Option<NaiveDate>,

        /// path to client secret file (the one you got from Google)
        #[argh(option, short = 'c')]
        client_secret: PathBuf,

        /// target folder (must *not* exist)
        #[argh(positional)]
        _target: PathBuf,
    }
    let config: Config = argh::from_env();
    debug!("{config:?}");

    //let

    // Get app secret
    let path = config
        .client_secret
        .to_str()
        .context("Invalid char in path to client_secret")?;
    let client_secret = fs::read_to_string(path).await?;
    let secret: oauth2::ConsoleApplicationSecret = serde_json::from_str(&client_secret)?;
    let app_secret = secret
        .installed
        .context("Wrong client secret format, expected `installed`")?;

    // Token storage module_path
    let store = "/tmp/goopho_tokens.json";

    // Client setup and auth
    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .enable_http2()
            .build(),
    );
    let auth = oauth2::InstalledFlowAuthenticator::builder(
        app_secret,
        oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .hyper_client(client.clone())
    .persist_tokens_to_disk(store)
    .build()
    .await?;

    // Ready for the real thing
    let hub = PhotosLibrary::new(client, auth);
    info!("Tokens stored to '{store}'");

    let mut next_page_token: Option<String> = None;
    loop {
        let result = if next_page_token.as_ref().is_some() {
            hub.media_items()
                .list()
                .page_token(&next_page_token.clone().unwrap())
                .page_size(100)
                .doit()
                .await
        } else {
            hub.media_items().list().page_size(100).doit().await
        };

        match result {
            Err(e) => match e {
                // The Error enum provides details about what exactly happened.
                // You can also just use its `Debug`, `Display` or `Error` traits
                Error::HttpError(_)
                | Error::Io(_)
                | Error::MissingAPIKey
                | Error::MissingToken(_)
                | Error::Cancelled
                | Error::UploadSizeLimitExceeded(_, _)
                | Error::Failure(_)
                | Error::BadRequest(_)
                | Error::FieldClash(_)
                | Error::JsonDecodeError(_, _) => println!("{}", e),
            },
            Ok(res) => {
                let (response, list_media_items_response) = res;
                if !response.status().is_success() {
                    error!("HTTP Not Ok {}...", response.status());
                } else {
                    next_page_token = list_batch(list_media_items_response, config.not_older);
                }
            }
        }
        if next_page_token.is_none() {
            break;
        }
    }

    Ok(())
}

fn list_batch(lmir: ListMediaItemsResponse, not_older: Option<NaiveDate>) -> Option<String> {
    if let Some(items) = lmir.media_items {
        let mut count: u8 = 0;
        let mut files: u8 = 0;

        items.iter().for_each(|item| {
            count += 1;
            if item.filename.is_some() {
                files += 1;

                if not_older.is_some()
                    && item.media_metadata.as_ref().is_some()
                    && item
                        .media_metadata
                        .as_ref()
                        .unwrap()
                        .creation_time
                        .is_some()
                {
                    // There's a way to select on creation date.
                    if item
                        .media_metadata
                        .as_ref()
                        .unwrap()
                        .creation_time
                        .unwrap()
                        .date_naive()
                        >= not_older.unwrap()
                    {
                        println!(
                            "{} {}",
                            item.media_metadata.as_ref().unwrap().creation_time.unwrap(),
                            item.filename.as_ref().unwrap()
                        );
                    }
                } else {
                    // There is no way to tell, and when in doubt, we select.
                    println!("{}", item.filename.as_ref().unwrap());
                };
            }
        });
        info!("Batch size: {count}, files: {files}");
    }
    lmir.next_page_token
}
