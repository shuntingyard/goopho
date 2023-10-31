//! Selection of media files to download

use google_photoslibrary1 as photoslibrary1;
use photoslibrary1::{
    api::{ListMediaItemsResponse, MediaItem, MediaMetadata},
    chrono::{DateTime, NaiveDate, Utc},
    hyper::client::HttpConnector,
    hyper_rustls::HttpsConnector,
    Error, PhotosLibrary,
};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Attributes of `MediaItem` to download
#[derive(Clone, Debug)]
pub enum MediaAttr {
    // URL, filename, width, height, creation time
    ImageOrMotionPhotoBaseUrl(String, String, i64, i64, DateTime<Utc>),
    // URL, filename, creation time
    VideoBaseUrl(String, String, DateTime<Utc>),
}

/// Collect attributes of `MediaItem`s to download and send on channel
pub async fn select_media_and_send(
    hub: PhotosLibrary<HttpsConnector<HttpConnector>>,
    transmit_to_write: mpsc::Sender<MediaAttr>,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
    batch_size: i32,
) -> anyhow::Result<()> {
    // Loop through Google Photos
    let mut next_page_token: Option<String> = None;
    loop {
        // First list in batches
        let result = if let Some(token) = next_page_token.as_ref() {
            hub.media_items()
                .list()
                .page_token(token)
                .page_size(batch_size)
                .doit()
                .await
        } else {
            hub.media_items().list().page_size(batch_size).doit().await
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
                    let (token_returned, selection) =
                        select_from_list(list_media_items_response, from_date, to_date);

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

    Ok(())
}

/// Handle pattern matching and time windows
fn select_from_list(
    response: ListMediaItemsResponse,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
) -> (Option<String>, Vec<MediaAttr>) {
    let mut selection = Vec::<MediaAttr>::new();
    let mut next_page_token = response.next_page_token;

    // TODO: Transform naive dates to Utc here!

    if let Some(items) = response.media_items {
        let mut total = 0;
        let mut selected_dt = 0;
        let mut skipped_dt = 0;
        let mut unexpected = 0;

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
                    mime_type: _,
                    product_url: _,
                } => match metadata {
                    MediaMetadata {
                        creation_time: Some(creation_time),
                        ..
                    } => {
                        // Do creation time selection if desired
                        let creation_date = creation_time.date_naive();
                        if from_date.is_some_and(|from_date| creation_date >= from_date)
                            && to_date.is_some_and(|to_date| creation_date <= to_date)
                            || (from_date.is_none()
                                && to_date.is_some_and(|to_date| creation_date <= to_date))
                            || (to_date.is_none()
                                && from_date.is_some_and(|from_date| creation_date >= from_date))
                            || (from_date.is_none() && to_date.is_none())
                        {
                            match metadata {
                                MediaMetadata {
                                    creation_time: _,
                                    height: Some(height),
                                    photo: Some(_),
                                    video: None,
                                    width: Some(width),
                                } => {
                                    selected_dt += 1;
                                    selection.push(MediaAttr::ImageOrMotionPhotoBaseUrl(
                                        url.to_string(),
                                        filename.to_string(),
                                        width.to_owned(),
                                        height.to_owned(),
                                        creation_time.to_owned(),
                                    ));
                                }
                                MediaMetadata {
                                    creation_time: _,
                                    height: _,
                                    photo: None,
                                    video: Some(_),
                                    width: _,
                                } => {
                                    selected_dt += 1;
                                    selection.push(MediaAttr::VideoBaseUrl(
                                        url.to_string(),
                                        filename.to_string(),
                                        creation_time.to_owned(),
                                    ));
                                }
                                _ => {
                                    unexpected += 1;
                                    warn!(
                                        "Refused to match without some photo or video {metadata:?}"
                                    );
                                }
                            }
                        } else {
                            skipped_dt += 1;
                            // End list early!
                            if from_date.is_some_and(|from_date| creation_date < from_date) {
                                next_page_token = None;
                            }
                        }
                    }
                    _ => {
                        unexpected += 1;
                        warn!("Refused to match without creation_time {metadata:?}");
                    }
                },
                _ => {
                    unexpected += 1;
                    warn!("Refused to match without all of: base_url, filename, metadata {item:?}");
                }
            }
        });
        info!(
            "Size: {total:2} selected: {selected_dt:2} skip: {skipped_dt:2} warn: {unexpected:2}"
        );
    }

    (next_page_token, selection)
}
