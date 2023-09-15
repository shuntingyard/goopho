use std::path::PathBuf;

use async_trait::async_trait;
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePoolOptions, Sqlite, SqlitePool};
use time;

use crate::calculations::Calculation;

/// Where we store calculations.

#[async_trait]
pub trait Store {
    async fn add(&self, mtime: time::OffsetDateTime, path: PathBuf, calculated: Vec<Calculation>);
    async fn contains(&self, mtime: time::OffsetDateTime, path: PathBuf) -> bool;
}

/// Simply write to stdout.
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

    /// A dummy alwaays returning `false`.
    async fn contains(&self, _: time::OffsetDateTime, _: PathBuf) -> bool {
        false
    }
}

/// This app's default store
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn build() -> Result<Self, Box<dyn std::error::Error>> {
        const DB_URL: &str = "./goopho.sl3";

        // Check if we have to initialize.
        if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
            Sqlite::create_database(DB_URL).await?
        }

        let pool = SqlitePoolOptions::new()
            //.min_connections(4)
            //.max_connections(8)
            .connect(DB_URL)
            .await?;

        Ok(Self { pool })
    }
}
