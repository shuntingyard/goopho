//! Acc(ounting) - keep track of completeness/failure

use std::{collections::HashSet, sync::Arc};

use futures::lock::Mutex;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Set of Google MediaItem IDs to process
pub struct _ToProcess {
    ids: Arc<Mutex<HashSet<String>>>,
}

/// Events to track
#[derive(Clone)]
pub enum Event {
    New,
    Retrying(String, u8),
    RetryAfter(String, u64),
    Failed(String),
    Completed,
    Summarize,
}

/// Count and log
pub async fn track_events(mut events: mpsc::Receiver<Event>) -> tokio::task::JoinHandle<()> {
    /// Internal details
    #[derive(Default)]
    struct Tracker {
        _total: i32,
        _completed: i32,
        _failed: i32,
    }

    tokio::spawn(async move {
        let mut mem = Tracker::default();

        while let Some(event) = events.recv().await {
            match event {
                Event::New => mem._total += 1,
                Event::Retrying(file, count) => warn!("Retried {file} {count} times ..."),
                Event::RetryAfter(to_shorten, t_msec) => {
                    let mut url = to_shorten;
                    url.truncate(48);
                    let pause: f32 = t_msec as f32 / 1000f32;
                    warn!("GET {url}...timeout, retry in {pause:2.2}s")
                }
                Event::Failed(file) => {
                    mem._failed += 1;
                    error!("Givin' up on {file} ...")
                }
                Event::Completed => mem._completed += 1,
                Event::Summarize => {
                    // assert_eq!(mem._total, mem._completed + mem._failed);
                    info!(
                        "Processed: total {}, completed {}, failed {}",
                        mem._total, mem._completed, mem._failed
                    );
                    break;
                }
            }
        }
    })
}
