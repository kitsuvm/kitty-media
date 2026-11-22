//! This module contains the task queue implementation used by Kitty Media server.

use migration_remote::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};

pub use sea_orm::DbErr;

#[allow(missing_docs, clippy::missing_docs_in_private_items)]
mod models;

/// Interface to the database containing the task queue.
pub struct TaskQueueDatabase {
    /// The database connection.
    db: DatabaseConnection,
}

impl TaskQueueDatabase {
    /// Creates a new instance of [`TaskQueueDatabase`] by connecting to the database using the provided connection string.
    pub async fn new(connection_string: &str) -> Result<Self, DbErr> {
        let db = Database::connect(connection_string).await?;

        migration_remote::Migrator::up(&db, None).await?;

        Ok(Self { db })
    }
}
