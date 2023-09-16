//! The persistence store implementation for Sqlite3

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sqlx::Row;
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePoolOptions, Sqlite, SqlitePool};
use time;
use tracing::debug;

use crate::calculations::Calculation;
use crate::persistence::Store;

/// This app's default store
pub struct SqliteStore {
    pool: SqlitePool,
}

#[async_trait]
impl Store for SqliteStore {
    /// Default store to use in simple cases.
    async fn add(&self, mtime: time::OffsetDateTime, url: PathBuf, calculated: Vec<Calculation>) {
        // Do we have to insert a row for `image`?
        let rowid = self.get_some_image_rowid(mtime, &url).await;

        let image_id: i64; // The PK for some inserts.

        if let Some(rowid) = rowid {
            image_id = rowid;
        } else {
            // Image not seen so far, we DO insert.
            sqlx::query(
                "insert into image (mtime, url, inserted) values ($1, $2, datetime('now'))",
            )
            .bind(&mtime.to_string())
            .bind(url.to_string_lossy())
            .execute(&self.pool)
            .await
            .unwrap();

            image_id = self.get_some_image_rowid(mtime, &url).await.unwrap(); // Unwrap here, as we
                                                                              // just inserted!
        }

        // Now do them all - with async IO.
        for calc in calculated {
            match calc {
                Calculation::Dhash(dhash) => {
                    sqlx::query("insert into dhash (image_id, dhash, inserted) values ($1, $2, datetime('now'))")
                        .bind(image_id)
                        .bind(dhash as i64) // TODO Think again if this coercion has any bad side
                        // effects, e.g. dumping from Sqlite and storing in Postgresql?
                        .execute(&self.pool)
                        .await
                        .unwrap();
                }
                Calculation::Thumbnail => {}
            }
        }
    }

    /// See if we have a table entry corresponding to the question.
    async fn contains(
        &self,
        mtime: time::OffsetDateTime,
        url: PathBuf,
        table: Calculation,
    ) -> bool {
        // First see if there is a row for this in image.
        let rowid = self.get_some_image_rowid(mtime, &url).await;

        if let Some(rowid) = rowid {
            // If so, check for the detail requested.
            sqlx::query("select image_id from $1 where image_id == $2")
                .bind(table.as_ref())
                .bind(rowid)
                .fetch_optional(&self.pool)
                .await
                .unwrap()
                .is_some()
        } else {
            false
        }
    }
}

impl SqliteStore {
    pub async fn build() -> Result<Self, Box<dyn std::error::Error>> {
        // TODO Use `directories` crate for a sensible location.
        const DB_URL: &str = "sqlite://goopho.sl3";

        // Check if we have to initialize.
        if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
            debug!("Pre migrations: creating DB {DB_URL}");
            Sqlite::create_database(DB_URL).await?
        }

        let pool = SqlitePoolOptions::new()
            //.min_connections(4)
            //.max_connections(8)
            .connect(DB_URL)
            .await?;

        sqlx::migrate!()
            .run(&pool)
            .await
            .expect("Migrations: failed running migrate.");

        Ok(Self { pool })
    }

    /// Modularization: we only code existence check and retrieval of `rowid` for `image` once, here!
    async fn get_some_image_rowid(&self, mtime: time::OffsetDateTime, url: &Path) -> Option<i64> {
        let row = sqlx::query("select rowid from image where mtime == $1 and url == $2")
            .bind(&mtime.to_string())
            .bind(&url.to_string_lossy())
            .fetch_optional(&self.pool)
            .await
            .unwrap();

        if let Some(rowid) = row {
            let rowid: i64 = rowid.try_get(0).unwrap();
            Some(rowid)
        } else {
            None
        }
    }
}
