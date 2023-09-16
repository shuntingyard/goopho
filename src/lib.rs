//! Walk a directory containing images recursively, calculate computer vision
//! things (e.g. perceptual hashes) for the directory's content and store (or
//! display) resulting data in a variety of ways.

use std::path::PathBuf;

use async_walkdir::WalkDir;
use futures_lite::stream::StreamExt;
use image::DynamicImage;
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};
use tracing::{debug, error, trace};

pub mod calculations;
pub mod persistence;

/// At the core of it all...
pub async fn walk_and_calculate(
    dir: PathBuf,
    store: impl persistence::Store + 'static,
    calculations: Vec<calculations::CalcFn>,
    fa: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // for scheduling and persistence
    let scheduler = SchedulerProxy::new(calculations, Box::new(store));

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
                                    scheduler
                                        .schedule_and_store(img, mtime, dir_entry.path())
                                        .await
                                }
                                Err(e) => error!("{e} ({})", dir_entry.path().display()),
                            }
                        } else {
                            let mut img = File::open(dir_entry.path()).await?;
                            let mut img_buf: Vec<u8> = vec![];
                            img.read_to_end(&mut img_buf).await?;

                            match image::load_from_memory(&img_buf) {
                                Ok(img) => {
                                    scheduler
                                        .schedule_and_store(img, mtime, dir_entry.path())
                                        .await
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

    debug!("*** Walk ended ***");
    Ok(())
}

struct SchedulerProxy {
    calculations: Vec<calculations::CalcFn>,
    store: Box<dyn persistence::Store>,
}

impl SchedulerProxy {
    fn new(calculations: Vec<calculations::CalcFn>, store: Box<dyn persistence::Store>) -> Self {
        Self {
            calculations,
            store,
        }
    }

    async fn schedule_and_store(
        &self,
        img: DynamicImage,
        mtime: time::OffsetDateTime,
        path: PathBuf,
    ) {
        let calculated = self
            .calculations
            .iter()
            .filter(|_| true) // FIXME
            // 1) We cannot do async calls (e.g.) to self.store.contains(...) here.
            // 2) Get calcfn's concrete fn to query store!
            .map(|calcfn| calcfn(&img))
            .collect();
        self.store.add(mtime, path, calculated).await;
    }
}
