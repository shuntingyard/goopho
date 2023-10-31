//! Attempt to linit concurrent downloads

use std::path::PathBuf;

use futures::{self, StreamExt};
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    hyper::{self, client::HttpConnector},
    hyper_rustls::HttpsConnector,
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::download::download_and_write;
use crate::hub::MediaAttr;

// Spawn green threads with some control over concurrency
pub async fn photos_to_disk(
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
