//! Handling of (potentially massive) asynchronous downloads and disk writes

use std::{path::PathBuf, str::FromStr};

use async_recursion::async_recursion;
use futures::{self, future, StreamExt};
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    hyper::{self, body::HttpBody, client::HttpConnector, StatusCode},
    hyper_rustls::HttpsConnector,
};
use tokio::{
    fs,
    io::{self, AsyncWriteExt},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::error;
use tracing::instrument;

use crate::hub::MediaAttr;

const IN_PROGRESS_SUFFIX: &str = ".chunks";

/// Spawn green threads to do the heavy lifting
pub async fn photos_to_disk(
    mut write_request: mpsc::Receiver<MediaAttr>,
    download_dir: PathBuf,
    client: hyper::Client<HttpsConnector<HttpConnector>>,
    is_dry_run: bool,
) -> JoinHandle<anyhow::Result<()>> {
    // Schedule downloads and disk writes
    tokio::spawn(async move {
        let mut handles = vec![];

        while let Some(item) = write_request.recv().await {
            let (url, filename, creation_time) = match item {
                MediaAttr::ImageOrMotionPhotoBaseUrl(url, name, width, height, ctime) => {
                    (url + &format!("=w{width}-h{height}"), name, ctime)
                }
                MediaAttr::VideoBaseUrl(url, name, ctime) => (url + "=dv", name, ctime),
            };
            let mut path = download_dir.clone();
            let http_cli = client.clone();

            let write_thread: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
                path.push(&filename);
                if is_dry_run {
                    println!("dry_run,\"{creation_time}\",{path:?}");
                } else {
                    download_and_write(http_cli, url, path).await?;
                }

                Ok(())
            });

            handles.push(write_thread);
        }

        future::try_join_all(handles).await?;
        Ok(())
    })
}

// Spawn green threads with some control over concurrency (experimental)
pub async fn photos_to_disk_buf_unord(
    mut write_request: mpsc::Receiver<MediaAttr>,
    download_dir: PathBuf,
    client: hyper::Client<HttpsConnector<HttpConnector>>,
    is_dry_run: bool,
) -> JoinHandle<anyhow::Result<()>> {
    // Schedule downloads and disk writes
    tokio::spawn(async move {
        let mut media_items = Vec::<MediaAttr>::new();
        while let Some(item) = write_request.recv().await {
            media_items.push(item);
        }
        let fetches = futures::stream::iter(media_items.into_iter().map(|item| {
            let (url, filename, creation_time) = match item {
                MediaAttr::ImageOrMotionPhotoBaseUrl(url, name, width, height, ctime) => {
                    (url + &format!("=w{width}-h{height}"), name, ctime)
                }
                MediaAttr::VideoBaseUrl(url, name, ctime) => (url + "=dv", name, ctime),
            };
            let mut path = download_dir.clone();
            let http_cli = client.clone();

            async move {
                path.push(&filename);
                if is_dry_run {
                    println!("dry_run,\"{creation_time}\",{path:?}");
                    Ok(())
                } else {
                    download_and_write(http_cli, url, path).await
                }
            }
        }))
        .buffer_unordered(20)
        .collect::<Vec<anyhow::Result<()>>>();

        fetches.await;
        Ok(())
    })
}

/// Used with progress indicator
#[instrument(name = "downloading", skip(http_cli, url))]
#[async_recursion]
async fn download_and_write(
    http_cli: hyper::Client<HttpsConnector<HttpConnector>>,
    url: String,
    path: PathBuf,
) -> anyhow::Result<()> {
    let mut res = http_cli.get(hyper::Uri::from_str(&url)?).await?;

    // Check HTTP status codes
    match res.status() {
        StatusCode::OK => {
            let chunks_written = path.to_string_lossy().to_string() + IN_PROGRESS_SUFFIX;
            let mut outfile = io::BufWriter::new(fs::File::create(&chunks_written).await?);

            while let Some(chunk) = res.body_mut().data().await {
                outfile.write_all(&chunk?).await?;
            }
            outfile.flush().await?;
            fs::rename(&chunks_written, &path).await?;
            // eprintln!("Wrote {path:?}");
        }
        StatusCode::FOUND => {
            if let Some(header) = res.headers().get("location") {
                let location = header.to_str()?;
                // Recursion
                download_and_write(http_cli, location.to_string(), path).await?;
            } else {
                error!(
                    "{path:?} not downloaded - couldn't get location after HTTP 302, headers: {:?}",
                    res.headers()
                );
            }
        }
        // Catch all
        _ => {
            error!("Got HTTP {}", res.status());
        }
    }

    Ok(())
}
