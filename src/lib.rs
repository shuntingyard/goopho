use std::path::PathBuf;

use async_walkdir::WalkDir;
use futures_lite::stream::StreamExt;
use time;
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};
use tracing::{debug, error, info, trace};

pub mod calculations;
pub mod persistence;

pub async fn walk_and_calculate(
    dir: PathBuf,
    store: impl persistence::Store,
    calculations: Vec<calculations::CalcFn>,
    fa: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Make path absolute.
    let absolute_path = fs::canonicalize(dir).await?;

    // Get image files.
    let mut entries = WalkDir::new(absolute_path);
    // Found no way to fully profit from Iter here (e.g.
    // - no async closures,
    // - difficult or impossible to collect stream),
    // so we go with a loop.
    loop {
        let entry = entries.next().await;
        if let Some(result) = entry {
            match result {
                Ok(dir_entry) => {
                    if dir_entry.file_type().await?.is_file() {
                        // Get key part 1.
                        let mtime: time::OffsetDateTime = fs::metadata(dir_entry.path())
                            .await?
                            .modified()
                            .expect("OS platform must support mtime for this app")
                            .into();

                        if !fa {
                            // Meaning we're not full async!
                            // Danger zone: synchronous IO
                            match image::open(dir_entry.path()) {
                                //match image::image_dimensions(dir_entry.path()) {
                                Ok(img) => {
                                    let calculated =
                                        calculations.iter().map(|calcfn| calcfn(&img)).collect();

                                    store.add(mtime, dir_entry.path(), calculated).await;
                                }
                                Err(e) => error!("{e} ({})", dir_entry.path().display()),
                            }
                        } else {
                            let mut img = File::open(dir_entry.path()).await?;
                            let mut img_buf: Vec<u8> = vec![];
                            img.read_to_end(&mut img_buf).await?;

                            match image::load_from_memory(&img_buf) {
                                Ok(img) => {
                                    let calculated =
                                        calculations.iter().map(|calcfn| calcfn(&img)).collect();

                                    store.add(mtime, dir_entry.path(), calculated).await;
                                }

                                Err(e) => error!("{e} ({})", dir_entry.path().display()),
                            }
                        }
                    } else {
                        trace!("Discarded on walk: {}", dir_entry.path().display());
                    }
                }
                Err(e) => debug!("Error walking: {e}"),
            }
        } else {
            break;
        }
    }

    Ok(())
}
