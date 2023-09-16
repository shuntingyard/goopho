//! Module doc for stores/persistence...

use std::path::PathBuf;

use async_trait::async_trait;
use time;

use crate::calculations::Calculation;

mod sqlite;
pub use sqlite::SqliteStore;

/// How we store calculations.
#[async_trait]
pub trait Store {
    async fn add(&self, mtime: time::OffsetDateTime, path: PathBuf, calculated: Vec<Calculation>);
    async fn contains(
        &self,
        mtime: time::OffsetDateTime,
        path: PathBuf,
        calculated: Calculation,
    ) -> bool;
}

/// Simply write to stdout; don't store anything.
pub struct StdoutStore;

#[async_trait]
impl Store for StdoutStore {
    /// This simply prints.
    async fn add(&self, mtime: time::OffsetDateTime, path: PathBuf, calculated: Vec<Calculation>) {
        println!(
            "mtime: {mtime}, path: {} {calculated:#?}",
            path.to_string_lossy()
        );
    }

    /// A dummy always returning `false`.
    async fn contains(&self, _: time::OffsetDateTime, _: PathBuf, _: Calculation) -> bool {
        false
    }
}
