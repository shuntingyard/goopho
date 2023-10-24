use std::path::PathBuf;

use anyhow::{bail, Context};
use argh::FromArgs;
use google_photoslibrary1 as photoslibrary1;
use microxdg::Xdg;
use photoslibrary1::{
    api::{ListMediaItemsResponse, MediaItem},
    chrono::{DateTime, NaiveDate, Utc},
    hyper, hyper_rustls, oauth2, Error, PhotosLibrary,
};
use tokio::fs;
use tracing::{debug, error, info, warn};
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
        target: PathBuf,
    }
    let config: Config = argh::from_env();
    debug!("{config:?}");

    // Path to token store
    let data_dir = Xdg::new()?.data()?;
    let mut store = data_dir;
    store.push("goopho-tokens.json");
    let store = store
        .to_str()
        .context("Invalid char in path to token store")?;

    // Get app secret
    let path = config
        .client_secret
        .to_str()
        .context("Invalid char in path to client secret")?;
    let client_secret = fs::read_to_string(path).await?;
    let secret: oauth2::ConsoleApplicationSecret = serde_json::from_str(&client_secret)?;
    let app_secret = secret
        .installed
        .context("Wrong client secret format, expected `installed`")?;

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

    // See about the target directory
    if fs::metadata(config.target).await.is_ok() {
        bail!("Target dir exists");
    }

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
                    let (token_returned, selection) =
                        select_from_list(list_media_items_response, config.not_older);

                    next_page_token = token_returned;
                    if selection.len() > 0 {
                        dbg!(selection);
                    }
                }
            }
        }
        if next_page_token.is_none() {
            break;
        }
    }

    Ok(())
}

#[derive(Debug)]
enum Selection {
    // id, filename, mime-type
    WithCreateTime(String, String, String, DateTime<Utc>),
    NoCreateTime(String, String, String), // A precaution, don't know if this ever occurs?
}

fn select_from_list(
    response: ListMediaItemsResponse,
    not_older: Option<NaiveDate>,
) -> (Option<String>, Vec<Selection>) {
    let mut selection = Vec::<Selection>::new();

    if let Some(items) = response.media_items {
        let mut total: u8 = 0;
        let mut selected_dt: u8 = 0;
        let mut skipped_dt: u8 = 0;
        let mut no_dt: u8 = 0;
        let mut incomplete: u8 = 0;

        items.iter().for_each(|item| {
            total += 1;
            match item {
                MediaItem {
                    base_url: _,
                    contributor_info: _,
                    description: _,
                    filename: Some(filename),
                    id: Some(id),
                    media_metadata: Some(metadata),
                    mime_type: Some(mime_type),
                    product_url: _,
                } => match metadata.creation_time {
                    Some(dt) => match not_older {
                        Some(limit) => {
                            if dt.date_naive() >= limit {
                                selected_dt += 1; // Selected because within limits
                                selection.push(Selection::WithCreateTime(
                                    id.to_string(),
                                    filename.to_string(),
                                    mime_type.to_string(),
                                    dt,
                                ));
                            } else {
                                skipped_dt += 1;
                            }
                        }
                        None => {
                            selected_dt += 1; // Selected as there was no limit
                            selection.push(Selection::WithCreateTime(
                                id.to_string(),
                                filename.to_string(),
                                mime_type.to_string(),
                                dt,
                            ));
                        }
                    },
                    None => {
                        no_dt += 1;
                        selection.push(Selection::NoCreateTime(
                            id.to_string(),
                            filename.to_string(),
                            mime_type.to_string(),
                        ))
                    }
                },
                _ => {
                    incomplete += 1;
                    warn!("Incomplete MediaItem: {item:?}")
                }
            }
        });
        info!(
            "Size: {total} selected: {} skip: {skipped_dt} warn: {incomplete}",
            selected_dt + no_dt
        );
    }
    (response.next_page_token, selection)
}
