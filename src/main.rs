use std::{path::PathBuf, str::FromStr};

use anyhow::{bail, Context};
use argh::FromArgs;
use google_photoslibrary1 as photoslibrary1;
use microxdg::Xdg;
use photoslibrary1::{
    api::{ListMediaItemsResponse, MediaItem},
    chrono::{DateTime, NaiveDate, Utc},
    hyper, hyper_rustls, oauth2, Error, PhotosLibrary,
};
use tokio::io::{self, AsyncWriteExt};
use tokio::{fs, sync::mpsc};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

const BATCH_SIZE: i32 = 100;
const QUEUE_DEPTH: usize = 10;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Subscribe to traces
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()) // Read trace levels from RUST_LOG env var
        .init();

    // Command line args used
    #[derive(FromArgs, Debug)]
    /// Download images and videos from Google Photos
    struct Config {
        /// just show what would be written
        #[argh(switch, short = 'd')]
        dry_run: bool,

        /// don't select media files created earlier (year-month-day)
        #[argh(option, short = 'f')]
        from_date: Option<NaiveDate>,

        /// don't select media files created later
        #[argh(option, short = 't')]
        to_date: Option<NaiveDate>,

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
    let home_data = Xdg::new()?.data()?;
    let mut store = home_data;
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
    info!("Tokens stored to '{store}'");

    // Ready for the real thing
    let hub = PhotosLibrary::new(client.clone(), auth);

    // See about the target directory
    if fs::metadata(&config.target).await.is_ok() {
        bail!("Target dir exists");
    } else {
        if !config.dry_run {
            fs::create_dir(&config.target).await?;
        }
    }

    // Channel to writers
    let (transmit_to_write, mut write_request) = mpsc::channel::<Selection>(QUEUE_DEPTH);

    // Schedule downloads and disk writes
    let writer = tokio::spawn(async move {
        while let Some(item) = write_request.recv().await {
            let (url, filename) = match item {
                Selection::WithCreateTime(u, n, _m, _d) => (u, n),
                Selection::NoCreateTime(u, n, _m) => (u, n),
            };
            let mut path = config.target.clone();
            let http_cli = client.clone();

            let write_thread = tokio::spawn(async move {
                path.push(&filename);
                if config.dry_run {
                    println!(" Pretenting write to {path:?}");
                } else {
                    let res = http_cli
                        .get(hyper::Uri::from_str(&url).unwrap())
                        .await
                        .unwrap();
                    // dbg!(&res);

                    println!(" About to write {path:?}");
                    // TODO: Improve this with reading in chunks and a buffered writer.
                    let mut output = fs::File::create(path).await.unwrap();
                    let buf = hyper::body::to_bytes(res).await.unwrap();
                    output.write(&buf[..]).await.unwrap();
                }
            });
            write_thread.await.unwrap();
        }
    });

    // Loop through Google Photos
    let mut next_page_token: Option<String> = None;
    loop {
        // First list in batches
        let result = if next_page_token.as_ref().is_some() {
            hub.media_items()
                .list()
                .page_token(&next_page_token.clone().unwrap())
                .page_size(BATCH_SIZE)
                .doit()
                .await
        } else {
            hub.media_items().list().page_size(BATCH_SIZE).doit().await
        };

        match result {
            Err(e) => match e {
                // The Error enum provides details about what exactly happened
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
                | Error::JsonDecodeError(_, _) => error!("{}", e),
            },
            Ok(res) => {
                let (response, list_media_items_response) = res;
                if !response.status().is_success() {
                    error!("HTTP Not Ok {}...", response.status());
                } else {
                    let (token_returned, selection) = select_from_list(
                        list_media_items_response,
                        config.from_date,
                        config.to_date,
                    );

                    next_page_token = token_returned;
                    for item in selection {
                        transmit_to_write.send(item.to_owned()).await?;
                    }
                }
            }
        }

        if next_page_token.is_none() {
            drop(transmit_to_write); // Close this end of the channel
            break;
        }
    }

    // Be patient, don't quit
    writer.await?;

    Ok(())
}

#[derive(Clone, Debug)]
enum Selection {
    // base_url, filename, mime-type
    WithCreateTime(String, String, String, DateTime<Utc>),
    NoCreateTime(String, String, String), // A precaution, don't know if this ever occurs?
}

fn select_from_list(
    response: ListMediaItemsResponse,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
) -> (Option<String>, Vec<Selection>) {
    let mut selection = Vec::<Selection>::new();
    let mut next_page_token = response.next_page_token;

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
                    base_url: Some(url),
                    contributor_info: _,
                    description: _,
                    filename: Some(filename),
                    id: _,
                    media_metadata: Some(metadata),
                    mime_type: Some(mime_type),
                    product_url: _,
                } => match metadata.creation_time {
                    Some(creation_time) => {
                        let creation_date = creation_time.date_naive();
                        if from_date.is_some_and(|from_date| creation_date >= from_date)
                            && to_date.is_some_and(|to_date| creation_date <= to_date)
                            || (from_date.is_none()
                                && to_date.is_some_and(|to_date| creation_date <= to_date))
                            || (to_date.is_none()
                                && from_date.is_some_and(|from_date| creation_date >= from_date))
                            || (from_date.is_none() && to_date.is_none())
                        {
                            selected_dt += 1; // Selected because within limits
                            selection.push(Selection::WithCreateTime(
                                url.to_string(),
                                filename.to_string(),
                                mime_type.to_string(),
                                creation_time,
                            ));
                        } else {
                            skipped_dt += 1;
                            if from_date.is_some_and(|from_date| creation_date < from_date) {
                                next_page_token = None;
                            }
                        }
                    }
                    None => {
                        no_dt += 1;
                        selection.push(Selection::NoCreateTime(
                            url.to_string(),
                            filename.to_string(),
                            mime_type.to_string(),
                        ));
                        warn!("Found MediaItem without creation time: {item:?}");
                    }
                },
                _ => {
                    incomplete += 1;
                    warn!("Incomplete MediaItem: {item:?}");
                }
            }
        });
        info!(
            "Size: {total} selected: {} skip: {skipped_dt} warn: {incomplete}",
            selected_dt + no_dt
        );
    }
    (next_page_token, selection)
}
