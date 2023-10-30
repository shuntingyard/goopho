//! Handling of (potentially massive) asynchronous downloads and disk writes

use std::{path::PathBuf, str::FromStr};

use futures::future;
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    hyper::{self, body::HttpBody, client::HttpConnector},
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
            // None for url in cases where image with or height are None
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

/// Used with progress indicator
#[instrument(name = "downloading", skip(http_cli, url))]
async fn download_and_write(
    http_cli: hyper::Client<HttpsConnector<HttpConnector>>,
    url: String,
    path: PathBuf,
) -> anyhow::Result<()> {
    let mut res = http_cli.get(hyper::Uri::from_str(&url)?).await?;
    // dbg!(&res);

    // Videos, 302?
    if res.status() == 200 {
        let mut output = io::BufWriter::new(fs::File::create(&path).await?);
        while let Some(chunk) = res.body_mut().data().await {
            output.write_all(&chunk?).await?;
        }
        // eprintln!("Wrote photo {path:?}");
    } else if res.status() == 302 {
        let location = res.headers().get("location").unwrap().to_str()?;
        let mut res = http_cli.get(hyper::Uri::from_str(location)?).await?;
        if res.status() == 200 {
            let mut output = io::BufWriter::new(fs::File::create(&path).await?);
            while let Some(chunk) = res.body_mut().data().await {
                output.write_all(&chunk?).await?;
            }
            // eprintln!("Wrote video {path:?}");
        }
    } else {
        error!("Got http {}", res.status());
    }

    Ok(())
}
