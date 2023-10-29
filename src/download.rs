//! Handling of (potentially massive) asynchronous downloads and disk writes

use std::{path::PathBuf, str::FromStr};

use futures::future;
use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    hyper::{self, body::HttpBody, client::HttpConnector},
    hyper_rustls::HttpsConnector,
};
use tokio::io::{self, AsyncWriteExt};
use tokio::{fs, sync::mpsc};
use tracing::{error, warn};

use crate::hub::Selection;

pub async fn photos_to_disk(
    mut write_request: mpsc::Receiver<Selection>,
    download_dir: PathBuf,
    client: hyper::Client<HttpsConnector<HttpConnector>>,
    is_dry_run: bool,
) -> tokio::task::JoinHandle<()> {
    // Schedule downloads and disk writes
    tokio::spawn(async move {
        let mut handles = vec![];

        while let Some(item) = write_request.recv().await {
            let _item_to_display = item.clone();

            // None for url in cases where image with or height are None
            let (url, filename) = match item {
                Selection::ImageOrMotionPhotoBaseUrl(url, name, Some(width), Some(height), _) => {
                    (url + &format!("=w{width}-h{height}"), name)
                }
                Selection::ImageOrMotionPhotoBaseUrl(url, name, _, _, _) => {
                    warn!("No dimensions for {name} - thumnail downloaded");
                    (url, name)
                }
                Selection::VideoBaseUrl(url, name, _) => (url + "=dv", name),
            };
            let mut path = download_dir.clone();
            let http_cli = client.clone();

            let write_thread = tokio::spawn(async move {
                path.push(&filename);
                if is_dry_run {
                    println!("\"dry_run\",{path:?}");
                    dbg!(url);
                    // dbg!(_item_to_display);
                } else {
                    let mut res = http_cli
                        .get(hyper::Uri::from_str(&url).unwrap())
                        .await
                        .unwrap();
                    // dbg!(&res);

                    // Videos, 302?
                    if res.status() == 200 {
                        let mut output = io::BufWriter::new(fs::File::create(&path).await.unwrap());
                        while let Some(chunk) = res.body_mut().data().await {
                            output.write_all(&chunk.unwrap()).await.unwrap();
                        }
                        println!("Wrote photo {path:?}");
                    } else if res.status() == 302 {
                        let location = res.headers().get("location").unwrap().to_str().unwrap();
                        let mut res = http_cli
                            .get(hyper::Uri::from_str(location).unwrap())
                            .await
                            .unwrap();
                        if res.status() == 200 {
                            let mut output =
                                io::BufWriter::new(fs::File::create(&path).await.unwrap());
                            while let Some(chunk) = res.body_mut().data().await {
                                output.write_all(&chunk.unwrap()).await.unwrap();
                            }
                            println!("Wrote video {path:?}");
                        }
                    } else {
                        error!("Got http {}", res.status());
                    }
                }
            });
            handles.push(write_thread);
        }
        future::join_all(handles).await;
    })
}
