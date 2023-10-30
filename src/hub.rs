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
    // URL, filename, width, height, optional creation time
    ImageOrMotionPhotoBaseUrl(
        String,
        String,
        Option<i64>,
        Option<i64>,
        Option<DateTime<Utc>>,
    ),
    // URL, filename, optional creation time
    VideoBaseUrl(String, String, Option<DateTime<Utc>>),
}

/// Attributes of `MediaItem` to download
#[derive(Clone, Debug)]
pub enum MediaAttrBetter {
    // URL, filename, width, height, optional creation time
    _ImageOrMotionPhotoBaseUrl(String, String, i64, i64, DateTime<Utc>),
    // URL, filename, optional creation time
    _VideoBaseUrl(String, String, DateTime<Utc>),
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
        let result = if next_page_token.as_ref().is_some() {
            hub.media_items()
                .list()
                .page_token(&next_page_token.clone().unwrap())
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
                        /*
                        let id = match &item {
                            Selection::ImageOrMotionPhotoBaseUrl(id, ..) => id,
                            Selection::VideoBaseUrl(id, ..) => id,
                        };
                        let result = hub
                            .media_items()
                            .get(id)
                            // .param("alt", "media")
                            .doit()
                            .await;

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
                                let (response, media_item) = res;
                                if !response.status().is_success() {
                                    error!("HTTP Not Ok {}...", response.status());
                                } else {
                                    if let Some(url) = media_item.base_url {
                                        // Enum rewriting crimes
                                        let item = match item {
                                            Selection::ImageOrMotionPhotoBaseUrl(_, b, c, d, e) => {
                                                Selection::ImageOrMotionPhotoBaseUrl(
                                                    url, b, c, d, e,
                                                )
                                            }
                                            Selection::VideoBaseUrl(_, b, c) => {
                                                Selection::VideoBaseUrl(url, b, c)
                                            }
                                        };
                                        transmit_to_write.send(item.to_owned()).await?;
                                    }
                                } // We don't handle errors redundantly, as a rewite is imminent now!
                            }
                        }
                        */
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

    if let Some(items) = response.media_items {
        let mut total: u8 = 0;
        let mut selected_dt: u8 = 0;
        let mut skipped_dt: u8 = 0;
        let mut no_dt: u8 = 0;
        let mut unexpected: u8 = 0;

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
                            // Photos
                            if metadata.photo.is_some() {
                                selected_dt += 1; // Selected because within limits
                                selection.push(MediaAttr::ImageOrMotionPhotoBaseUrl(
                                    url.to_string(),
                                    filename.to_string(),
                                    metadata.width,
                                    metadata.height,
                                    Some(creation_time),
                                ));
                            }
                            // Video
                            else if metadata.video.is_some() {
                                selected_dt += 1; // Selected because within limits
                                selection.push(MediaAttr::VideoBaseUrl(
                                    url.to_string(),
                                    filename.to_string(),
                                    Some(creation_time),
                                ));
                            }
                            // Don't know (which should't happen)
                            else {
                                unexpected += 1;
                                warn!("Unexpected MediaItem: {item:?}");
                            }

                            // dbg!(&item);
                        } else {
                            skipped_dt += 1;
                            // End list!
                            if from_date.is_some_and(|from_date| creation_date < from_date) {
                                next_page_token = None;
                            }
                        }
                    }
                    None => {
                        // Photos
                        if metadata.photo.is_some() {
                            no_dt += 1;
                            selection.push(MediaAttr::ImageOrMotionPhotoBaseUrl(
                                url.to_string(),
                                filename.to_string(),
                                metadata.width,
                                metadata.height,
                                None,
                            ));
                        }
                        // Video
                        else if metadata.video.is_some() {
                            no_dt += 1;
                            selected_dt += 1; // Selected because within limits
                            selection.push(MediaAttr::VideoBaseUrl(
                                url.to_string(),
                                filename.to_string(),
                                None,
                            ));
                        }
                        // Don't know (which should't happen)
                        else {
                            unexpected += 1;
                            warn!("Unexpected (neither photo nor video) MediaItem: {item:?}");
                        }

                        // dbg!(&item);
                        warn!("Found MediaItem without creation time: {item:?}");
                    }
                },
                _ => {
                    unexpected += 1;
                    warn!("Unexpected (some attributes don't match) MediaItem: {item:?}");
                }
            }
        });
        info!(
            "Size: {total} selected: {} skip: {skipped_dt} warn: {unexpected}",
            selected_dt + no_dt
        );
    }
    (next_page_token, selection)
}

/// Handle pattern matching and time windows
fn _select_from_list_better(
    response: ListMediaItemsResponse,
    _from_date: Option<NaiveDate>,
    _to_date: Option<NaiveDate>,
) -> (Option<String>, Vec<MediaAttrBetter>) {
    let selection = Vec::<MediaAttrBetter>::new();
    let next_page_token = response.next_page_token;

    // TODO: Transform naive dates to Utc here!

    if let Some(items) = response.media_items {
        items.iter().for_each(|item| match item {
            MediaItem {
                base_url: Some(_url),
                contributor_info: _,
                description: _,
                filename: Some(_filename),
                id: _,
                media_metadata: Some(metadata),
                mime_type: _,
                product_url: _,
            } => match metadata {
                MediaMetadata {
                    creation_time: Some(_creation_time),
                    ..
                } => match metadata {
                    // Do creation time selection
                    MediaMetadata {
                        creation_time: _,
                        height: Some(_height),
                        photo: Some(_),
                        video: None,
                        width: Some(_width),
                    } => {}
                    MediaMetadata {
                        creation_time: _,
                        height: None,
                        photo: None,
                        video: Some(_),
                        width: None,
                    } => {}
                    _ => {
                        warn!("Refused to match without some photo or video {metadata:?}");
                    }
                },
                _ => {
                    warn!("Refused to match without creation_time {metadata:?}");
                }
            },
            _ => {
                warn!("Refused to match without all of: base_url, filename, metadata {item:?}");
            }
        })
    }

    (next_page_token, selection)
}
