//! Handling of (potentially massive) asynchronous downloads and disk writes

use std::{path::PathBuf, str::FromStr, time::Duration};

use async_recursion::async_recursion;
use futures::{self, future, StreamExt};
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    hyper::{self, body::HttpBody, client::HttpConnector, StatusCode},
    hyper_rustls::HttpsConnector,
};
use rand::Rng;
use tokio::{
    fs,
    io::{self, AsyncWriteExt},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::instrument;

use crate::{acc::Event, hub::MediaAttr};

const IN_PROGRESS_SUFFIX: &str = ".chunks";
const TIMEOUT_MS: u64 = 3000;

/// Spawn green threads to do the heavy lifting
pub async fn photos_to_disk(
    mut write_request: mpsc::Receiver<MediaAttr>,
    track_and_log: mpsc::Sender<Event>,
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
                    // (url + &format!("=w{width}-h{height}"), name, ctime)
                    (url + "=d", name, ctime)
                }
                MediaAttr::VideoBaseUrl(url, name, ctime) => (url + "=dv", name, ctime),
            };

            // TODO: Prepare this just before you're really about to download.
            let mut path = download_dir.clone();
            let http_cli = client.clone();
            let track_and_log = track_and_log.clone();
            let mut rng = rand::thread_rng();
            let sleep_seed = rng.gen_range(TIMEOUT_MS..(TIMEOUT_MS + TIMEOUT_MS / 2));

            let write_thread: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
                path.push(&filename);
                if is_dry_run {
                    println!("dry_run,\"{creation_time}\",{path:?}");
                } else {
                    track_and_log.send(Event::New).await?;
                    download_and_write(http_cli, url, path, track_and_log, sleep_seed).await?;
                }

                Ok(())
            });

            handles.push(write_thread);
        }

        future::try_join_all(handles).await?;
        Ok(())
    })
}

/// Spawn green threads with some control over concurrency (experimental)
pub async fn photos_to_disk_unordered(
    mut write_request: mpsc::Receiver<MediaAttr>,
    track_and_log: mpsc::Sender<Event>,
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

            // TODO: Prepare this just before you're really about to download.
            let mut path = download_dir.clone();
            let http_cli = client.clone();
            let track_and_log = track_and_log.clone();
            let mut rng = rand::thread_rng();
            let sleep_seed = rng.gen_range(TIMEOUT_MS..(TIMEOUT_MS + TIMEOUT_MS / 2));

            async move {
                path.push(&filename);
                if is_dry_run {
                    println!("dry_run,\"{creation_time}\",{path:?}");
                    Ok(())
                } else {
                    track_and_log.send(Event::New).await?;
                    download_and_write(http_cli, url, path, track_and_log, sleep_seed).await
                }
            }
        }))
        .buffer_unordered(100)
        .collect::<Vec<anyhow::Result<()>>>();

        fetches.await;
        Ok(())
    })
}

/// Used with progress indicator
#[instrument(name = "downloading", skip(http_cli, url, track_and_log, sleep_seed))]
#[async_recursion]
async fn download_and_write(
    http_cli: hyper::Client<HttpsConnector<HttpConnector>>,
    url: String,
    path: PathBuf,
    track_and_log: mpsc::Sender<Event>,
    sleep_seed: u64,
) -> anyhow::Result<()> {
    let uri = hyper::Uri::from_str(&url)?;

    // Timeout/retry
    let mut pause_ms = sleep_seed;

    let mut res;
    loop {
        match tokio::time::timeout(Duration::from_millis(TIMEOUT_MS), http_cli.get(uri.clone()))
            .await
        {
            Ok(response) => {
                res = response.unwrap(); // TODO: Handle connection resets from here.
                break;
            }
            Err(_) => {
                // Timeout branch
                track_and_log
                    .send(Event::RetryAfter(url.clone(), pause_ms))
                    .await?;
                tokio::time::sleep(Duration::from_millis(pause_ms)).await;
                pause_ms *= 2;
                continue;
            }
        }
    }

    // Check HTTP status codes
    match res.status() {
        StatusCode::OK => {
            let chunks_written = path.to_string_lossy().to_string() + IN_PROGRESS_SUFFIX;
            let mut outfile = io::BufWriter::new(fs::File::create(&chunks_written).await?);
            let mut timeouts: u8 = 0;
            let mut completed = false;

            let body = res.body_mut();
            loop {
                let chunk =
                    match tokio::time::timeout(Duration::from_millis(5000), body.data()).await {
                        Ok(next) => {
                            if let Some(chunk) = next {
                                chunk
                            } else {
                                completed = true;
                                break;
                            }
                        }
                        Err(_) => {
                            timeouts += 1;
                            if timeouts.rem_euclid(60u8) == 0 {
                                track_and_log
                                    .send(Event::Failed(chunks_written.clone()))
                                    .await?;
                                break;
                            } else if timeouts.rem_euclid(20u8) == 0 {
                                track_and_log
                                    .send(Event::Retrying(chunks_written.clone(), timeouts))
                                    .await?;
                            }
                            continue;
                        }
                    };
                outfile.write_all(&chunk?).await?;
            }

            outfile.flush().await?;
            if completed {
                fs::rename(&chunks_written, &path).await?;
                track_and_log.send(Event::Completed).await?;
            }
            // eprintln!("Wrote {path:?}");
        }
        StatusCode::FOUND => {
            if let Some(header) = res.headers().get("location") {
                let location = header.to_str()?;
                // Recursion
                download_and_write(
                    http_cli,
                    location.to_string(),
                    path,
                    track_and_log.clone(),
                    pause_ms,
                )
                .await?;
            } else {
                // When location in 302 was invalid:
                track_and_log
                    .send(Event::Failed(path.to_string_lossy().to_string()))
                    .await?;
            }
        }
        // Catch all
        _ => {
            track_and_log
                .send(Event::FailedHttp(
                    path.to_string_lossy().to_string(),
                    res.status().to_string(),
                ))
                .await?;
        }
    }

    Ok(())
}
