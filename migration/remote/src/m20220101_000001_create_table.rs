use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("tasks")
                    .if_not_exists()
                    .col(big_integer("id").not_null().primary_key())
                    .col(date_time_default_now("created_at"))
                    .col(string("input").not_null())
                    .col(string("output").not_null())
                    .col(binary("options").not_null())
                    .col(date_time_null("processing_at"))
                    .col(date_time_null("completed_at"))
                    .col(tiny_integer_null("attempts").not_null().default(0))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-tasks-processing_at")
                    .table("tasks")
                    .col("processing_at")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-tasks-completed_at")
                    .table("tasks")
                    .col("completed_at")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-tasks-created_at")
                    .table("tasks")
                    .col("created_at")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("tasks").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-tasks-processing_at")
                    .table("tasks")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-tasks-completed_at")
                    .table("tasks")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-tasks-created_at")
                    .table("tasks")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
