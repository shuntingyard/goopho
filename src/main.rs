//! What to describe here?

use anyhow::{bail, Context};
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{hyper, hyper_rustls, oauth2, PhotosLibrary};
use tokio::{fs, sync::mpsc};
use tracing::{debug, info};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

mod config;
mod download;
mod hub;

const BATCH_SIZE: i32 = 50;
const QUEUE_DEPTH: usize = 10;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Subscribe to traces
    /*
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()) // Read trace levels from RUST_LOG env var
        .init();
     */
    console_subscriber::init();

    // Get command line args
    let args: config::Cmdlargs = argh::from_env();
    debug!("{args:?}");

    // Path to token store
    let store = config::get_token_store_path()?
        .to_str()
        .context("Invalid char in path to token store")?
        .to_string();

    // Get app secret
    //  (It's simply a design choice here to make this mandatory. Apps like `rclone`
    //  store this to a config file.)
    let app_secret = config::get_app_secret(args.client_secret).await?;

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
    .persist_tokens_to_disk(&store)
    .build()
    .await?;
    info!("Tokens stored to '{store}'");

    // Ready for the real thing
    let hub = PhotosLibrary::new(client.clone(), auth);

    // See about the target directory
    //  TODO: Only run  this code before actually writing
    if fs::metadata(&args.target).await.is_ok() {
        bail!("Target dir exists");
    } else if !args.dry_run {
        fs::create_dir(&args.target).await?;
    }

    // Channel to writers
    let (transmit_to_write, write_request) = mpsc::channel::<hub::Selection>(QUEUE_DEPTH);

    // Set up the channel's receiving side for downloads and disk writes
    //  (Manages its own join handles internally)
    let writer = download::photos_to_disk(write_request, args.target, client, args.dry_run).await;

    // Start selecting media files to download
    hub::select_media_and_send(
        hub,
        transmit_to_write,
        args.from_date,
        args.to_date,
        BATCH_SIZE,
    )
    .await?;

    // Be patient, don't quit
    // And don't forget to treat outer (?) as well as inner (?) results
    writer.await??;
    Ok(())
}
