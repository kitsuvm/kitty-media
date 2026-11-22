//! This module contains the task queue implementation used by Kitty Media server.

use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use migration_remote::{LockBehavior, LockType, MigratorTrait};
use models::prelude::*;
use sea_orm::{
    ActiveValue::Set, Condition, Database, QueryOrder, QuerySelect, TransactionError,
    TransactionTrait, prelude::*,
};

pub use sea_orm::DbErr;
use snowflaked::Generator;
use time::{UtcDateTime, macros::utc_datetime};

use crate::{models::Task, task_queue::models::tasks};

/// The epoch used for generating unique task IDs. This is set to January 1, 2026, at 00:00 UTC.
pub const KITTY_MEDIA_EPOCH: UtcDateTime = utc_datetime!(2026-01-01 0:00);

#[allow(missing_docs, clippy::missing_docs_in_private_items)]
mod models;

/// Interface to the database containing the task queue.
pub struct TaskQueueDatabase {
    /// The database connection.
    db: DatabaseConnection,
    /// The ID generator for creating unique task IDs.
    id_gen: Mutex<Generator>,
}

impl TaskQueueDatabase {
    /// Creates a new instance of [`TaskQueueDatabase`] by connecting to the database using the provided connection string.
    pub async fn new(connection_string: &str, machine_id: u16) -> Result<Self, DbErr> {
        let id_gen = Mutex::new(
            Generator::builder()
                .epoch(KITTY_MEDIA_EPOCH.into())
                .instance_checked(machine_id)
                .map(|v| v.build())
                .ok_or(DbErr::Custom("Invalid machine id".to_owned()))?,
        );

        let db = Database::connect(connection_string).await?;

        migration_remote::Migrator::up(&db, None).await?;

        Ok(Self { db, id_gen })
    }

    /// Inserts a new task into the task queue database.
    pub async fn insert(&self, task: &Task) -> Result<(), DbErr> {
        let options_bytes = rmp_serde::to_vec(&task.options).map_err(|e| DbErr::TryIntoErr {
            from: "kitty_media::models::Options",
            into: "std::vec::Vec<u8>",
            source: Arc::new(e),
        })?;

        let task_model = tasks::ActiveModel {
            id: Set(self
                .id_gen
                .lock()
                .map_err(|_| DbErr::MutexPoisonError)?
                .generate()),
            input: Set(task.input.clone()),
            output: Set(task.output.clone()),
            options: Set(options_bytes),
            ..Default::default()
        };

        task_model.insert(&self.db).await?;

        Ok(())
    }

    /// Fetches a list of tasks from the task queue database that are not currently being processed or completed, up to the specified limit.
    pub async fn fetch(&self, limit: u64) -> Result<Vec<(i64, Task)>, DbErr> {
        self.db
            .transaction(|tx| {
                Box::pin(async move {
                    let tasks = Tasks::find()
                        .filter(
                            Condition::all()
                                .add(tasks::Column::ProcessingAt.is_null())
                                .add(tasks::Column::CompletedAt.is_null()),
                        )
                        .order_by_asc(tasks::Column::CreatedAt)
                        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
                        .limit(limit)
                        .all(tx)
                        .await?;

                    let now = UtcDateTime::now();

                    for task in &tasks {
                        let mut task_model: tasks::ActiveModel = task.clone().into();
                        task_model.processing_at =
                            Set(Some(TimeDateTime::new(now.date(), now.time())));
                        task_model.update(tx).await?;
                    }

                    let mut task_list = Vec::with_capacity(tasks.capacity());

                    for task in tasks {
                        let options = match rmp_serde::from_slice(&task.options) {
                            Ok(v) => v,
                            Err(e) => {
                                return Err(DbErr::TryIntoErr {
                                    from: "std::vec::Vec<u8>",
                                    into: "kitty_media::models::Options",
                                    source: Arc::new(e),
                                });
                            }
                        };

                        task_list.push((
                            task.id,
                            Task {
                                input: task.input,
                                output: task.output,
                                options,
                            },
                        ));
                    }

                    Ok(task_list)
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(err) => err,
                TransactionError::Transaction(err) => err,
            })
    }

    pub async fn delete(&self, task_id: i64) -> Result<(), DbErr> {
        Tasks::delete_by_id(task_id).exec(&self.db).await?;

        Ok(())
    }

    pub async fn list(&self, limit: u32) -> Result<Vec<Task>, DbErr> {
        // Implementation for listing tasks from the database
        Ok(vec![])
    }

    pub async fn update_completed(&self, task_id: i64) -> Result<(), DbErr> {
        // Implementation for updating the completed status of a task
        Ok(())
    }

    pub async fn cleanup(&self) -> Result<(), DbErr> {
        // Implementation for cleaning up completed tasks from the database
        Ok(())
    }
}
