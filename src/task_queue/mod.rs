//! This module contains the task queue implementation used by Kitty Media server.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use migration_remote::{LockBehavior, LockType, Migrator, MigratorTrait};
use models::prelude::*;
use sea_orm::{
    ActiveValue::Set, Condition, Database, ExprTrait, QueryOrder, QuerySelect, TransactionError,
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
    /// The maximum number of attempts allowed for processing a task.
    max_attempts: u8,
}

impl TaskQueueDatabase {
    /// Creates a new instance of [`TaskQueueDatabase`] by connecting to the database using the provided connection string.
    pub async fn new(
        connection_string: &str,
        machine_id: u16,
        max_attempts: u8,
    ) -> Result<Self, DbErr> {
        let id_gen = Mutex::new(
            Generator::builder()
                .epoch(KITTY_MEDIA_EPOCH.into())
                .instance_checked(machine_id)
                .map(|v| v.build())
                .ok_or(DbErr::Custom("Invalid machine id".to_owned()))?,
        );

        let db = Database::connect(connection_string).await?;

        Migrator::up(&db, None).await?;

        Ok(Self {
            db,
            id_gen,
            max_attempts,
        })
    }

    /// Inserts a new task into the task queue database.
    pub async fn insert(&self, task: &Task) -> Result<i64, DbErr> {
        let options_bytes = rmp_serde::to_vec(&task.options).map_err(|e| DbErr::TryIntoErr {
            from: "kitty_media::models::Options",
            into: "std::vec::Vec<u8>",
            source: Arc::new(e),
        })?;

        let task_id = self.id_gen.lock().unwrap().generate();

        let task_model = tasks::ActiveModel {
            id: Set(task_id),
            input: Set(task.input.clone()),
            output: Set(task.output.clone()),
            options: Set(options_bytes),
            ..Default::default()
        };

        task_model.insert(&self.db).await?;

        Ok(task_id)
    }

    /// Fetches a list of tasks from the task queue database that are not currently being processed or completed, up to the specified limit.
    pub async fn fetch(&self, limit: u64) -> Result<Vec<(i64, Task)>, DbErr> {
        self.db
            .transaction(|tx| {
                let max_attempts = self.max_attempts;

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

                    if tasks.is_empty() {
                        return Ok(Vec::new());
                    }

                    let now = UtcDateTime::now();

                    let mut failed_tasks = Vec::new();

                    let task_list = tasks
                        .into_iter()
                        .filter_map(|task| match rmp_serde::from_slice(&task.options) {
                            Ok(v) => Some((
                                task.id,
                                Task {
                                    input: task.input,
                                    output: task.output,
                                    options: v,
                                },
                            )),
                            Err(e) => {
                                failed_tasks.push(task.id);
                                tracing::error!(
                                    "Failed to deserialize options for task {}: {e}",
                                    task.id
                                );
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    if !task_list.is_empty() {
                        Tasks::update_many()
                            .col_expr(
                                tasks::Column::ProcessingAt,
                                Expr::value(TimeDateTime::new(now.date(), now.time())),
                            )
                            .filter(tasks::Column::Id.is_in(task_list.iter().map(|(id, _)| *id)))
                            .exec(tx)
                            .await?;
                    }

                    if !failed_tasks.is_empty() {
                        tracing::warn!(
                            "Marking {} tasks as failed due to deserialization errors",
                            failed_tasks.len()
                        );

                        Tasks::update_many()
                            .col_expr(
                                tasks::Column::Attempts,
                                Expr::col(tasks::Column::Attempts).add(Expr::value(1)),
                            )
                            .filter(tasks::Column::Id.is_in(failed_tasks.iter()))
                            .exec(tx)
                            .await?;

                        Tasks::delete_many()
                            .filter(tasks::Column::Id.is_in(failed_tasks))
                            .filter(tasks::Column::Attempts.gte(max_attempts))
                            .exec(tx)
                            .await?;
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

    /// Deletes a task from the task queue database by its ID.
    pub async fn delete(&self, task_id: i64) -> Result<(), DbErr> {
        Tasks::delete_by_id(task_id)
            .exec(&self.db)
            .await
            .map(|_| ())
    }

    /// Lists tasks from the task queue database, up to the specified limit.
    pub async fn list(
        &self,
        limit: u64,
        offset: Option<i64>,
    ) -> Result<Vec<(i64, Result<Task, rmp_serde::decode::Error>)>, DbErr> {
        let mut tasks_query = Tasks::find().cursor_by(tasks::Column::Id);

        tasks_query.first(limit);

        if let Some(offset_id) = offset {
            tasks_query.after(offset_id);
        }

        tasks_query.all(&self.db).await.map(|tasks| {
            tasks
                .into_iter()
                .map(|task| match rmp_serde::from_slice(&task.options) {
                    Ok(v) => (
                        task.id,
                        Ok(Task {
                            input: task.input,
                            output: task.output,
                            options: v,
                        }),
                    ),
                    Err(e) => (task.id, Err(e)),
                })
                .collect()
        })
    }

    /// Updates the completed status of a task in the task queue database by its ID.
    pub async fn update_completed(&self, task_id: i64) -> Result<(), DbErr> {
        let now = UtcDateTime::now();

        Tasks::update_many()
            .col_expr(
                tasks::Column::CompletedAt,
                Expr::value(TimeDateTime::new(now.date(), now.time())),
            )
            .filter(tasks::Column::Id.eq(task_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    /// Marks tasks that have been processing for longer than the specified maximum duration as failed and deletes tasks that have exceeded the maximum number of attempts, up to the specified limit.
    async fn cleanup_stale(&self, max: Duration, limit: u64) -> Result<(), DbErr> {
        self.db
                .transaction(|tx| {
                    let max_attempts = self.max_attempts;

                    Box::pin(async move {
                        let now = UtcDateTime::now();

                        let processing_at_threshold = now - max;

                        let processing_ids = Tasks::find()
                            .select_only()
                            .column(tasks::Column::Id)
                            .column(tasks::Column::Attempts)
                            .filter(
                                Condition::all()
                                    .add(tasks::Column::CompletedAt.is_null())
                                    .add(tasks::Column::ProcessingAt.is_not_null())
                                    .add(tasks::Column::ProcessingAt.lt(TimeDateTime::new(
                                        processing_at_threshold.date(),
                                        processing_at_threshold.time(),
                                    ))),
                            )
                            .order_by_asc(tasks::Column::ProcessingAt)
                            .limit(limit)
                            .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
                            .into_tuple::<(i64, u8)>()
                            .all(tx)
                            .await?;

                        if !processing_ids.is_empty() {
                            tracing::warn!(
                                "Marking {} tasks for processing as failed due to exceeding processing time limit",
                                processing_ids.len()
                            );

                            let (retry, deleting) = processing_ids
                                .into_iter()
                                .partition::<Vec<_>, _>(|(_, attempts)| attempts.saturating_add(1) < max_attempts);

                            if !retry.is_empty() {
                                tracing::info!(
                                    "Retrying {} tasks that have not exceeded the maximum number of attempts",
                                    retry.len()
                                );

                                Tasks::update_many()
                                    .col_expr(
                                        tasks::Column::Attempts,
                                        Expr::col(tasks::Column::Attempts).add(Expr::value(1)),
                                    )
                                    .col_expr(tasks::Column::ProcessingAt, Expr::null())
                                    .filter(tasks::Column::Id.is_in(retry.iter().map(|(id, _)| *id)))
                                    .exec(tx)
                                    .await?;
                            }

                            if !deleting.is_empty() {
                                tracing::warn!(
                                    "Deleting {} tasks that have exceeded the maximum number of attempts",
                                    deleting.len()
                                );

                                Tasks::delete_many()
                                    .filter(tasks::Column::Id.is_in(deleting.iter().map(|(id, _)| *id)))
                                    .exec(tx)
                                    .await?;
                            }
                        }

                        Ok(())
                    })
                })
                .await
                .map_err(|e| match e {
                    TransactionError::Connection(err) => err,
                    TransactionError::Transaction(err) => err,
                })
    }

    /// Deletes tasks from the task queue database that have been completed for longer than the specified maximum duration, up to the specified limit.
    async fn cleanup_completed(&self, max: Duration, limit: u64) -> Result<(), DbErr> {
        self.db
                .transaction(|tx| {
                    Box::pin(async move {
                        let now = UtcDateTime::now();

                        let completed_at_threshold = now - max;

                        let completed_ids = Tasks::find()
                            .select_only()
                            .column(tasks::Column::Id)
                            .filter(
                                Condition::all()
                                    .add(tasks::Column::CompletedAt.is_not_null())
                                    .add(tasks::Column::CompletedAt.lt(TimeDateTime::new(
                                        completed_at_threshold.date(),
                                        completed_at_threshold.time(),
                                    ))),
                            )
                            .order_by_asc(tasks::Column::CompletedAt)
                            .limit(limit)
                            .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
                            .into_tuple::<i64>()
                            .all(tx)
                            .await?;

                        if !completed_ids.is_empty() {
                            tracing::info!(
                                "Deleting {} tasks that have been completed for longer than the specified limit",
                                completed_ids.len()
                            );

                            Tasks::delete_many()
                                .filter(tasks::Column::Id.is_in(completed_ids))
                                .exec(tx)
                                .await?;
                        }

                        Ok(())
                    })
                })
                .await
                .map_err(|e| match e {
                    TransactionError::Connection(err) => err,
                    TransactionError::Transaction(err) => err,
                })
    }

    /// Executes housecleaning operations on the task queue database, including marking tasks that have been processing for too long for a retry and deleting tasks that have been completed for too long.
    pub async fn cleanup(
        &self,
        processing_max: Duration,
        completed_max: Duration,
        limit: u64,
    ) -> Result<(), DbErr> {
        tokio::try_join!(
            self.cleanup_stale(processing_max, limit),
            self.cleanup_completed(completed_max, limit)
        )
        .map(|_| ())
    }
}
