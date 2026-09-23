use crate::StateRuntime;
use crate::state_db_path;
use chrono::Utc;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use uuid::Uuid;

const MAX_QUEUE_ITEMS: i64 = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedUserSubmissionRecord {
    pub id: String,
    pub thread_id: ThreadId,
    pub payload_json: String,
}

impl QueuedUserSubmissionRecord {
    fn try_from_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            thread_id: ThreadId::try_from(row.try_get::<String, _>("thread_id")?)?,
            payload_json: row.try_get("payload_json")?,
        })
    }
}

impl StateRuntime {
    async fn queue_pool(&self) -> anyhow::Result<SqlitePool> {
        let options = SqliteConnectOptions::new()
            .filename(state_db_path(self.codex_home()))
            .create_if_missing(false);
        Ok(SqlitePool::connect_with(options).await?)
    }

    pub async fn enqueue_user_submission(
        &self,
        thread_id: ThreadId,
        payload_json: &str,
    ) -> anyhow::Result<QueuedUserSubmissionRecord> {
        let pool = self.queue_pool().await?;
        let now_ms = Utc::now().timestamp_millis();
        let row = sqlx::query(
            "INSERT INTO queued_items (
                id, thread_id, payload_json, queue_order, created_at_ms, updated_at_ms
             )
             SELECT ?, ?, ?,
                    COALESCE((SELECT MAX(queue_order) FROM queued_items WHERE thread_id = ?), -1) + 1,
                    ?, ?
             WHERE (SELECT COUNT(*) FROM queued_items WHERE thread_id = ?) < ?
             RETURNING id, thread_id, payload_json",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(thread_id.to_string())
        .bind(payload_json)
        .bind(thread_id.to_string())
        .bind(now_ms)
        .bind(now_ms)
        .bind(thread_id.to_string())
        .bind(MAX_QUEUE_ITEMS)
        .fetch_one(&pool)
        .await?;
        let record = QueuedUserSubmissionRecord::try_from_row(&row)?;
        pool.close().await;
        Ok(record)
    }

    pub async fn queued_user_submissions(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Vec<QueuedUserSubmissionRecord>> {
        let pool = self.queue_pool().await?;
        let rows = sqlx::query(
            "SELECT id, thread_id, payload_json
             FROM queued_items
             WHERE thread_id = ?
             ORDER BY queue_order",
        )
        .bind(thread_id.to_string())
        .fetch_all(&pool)
        .await?;
        let records = rows
            .iter()
            .map(QueuedUserSubmissionRecord::try_from_row)
            .collect::<anyhow::Result<Vec<_>>>()?;
        pool.close().await;
        Ok(records)
    }

    pub async fn delete_queued_user_submission(
        &self,
        thread_id: ThreadId,
        item_id: &str,
    ) -> anyhow::Result<bool> {
        let pool = self.queue_pool().await?;
        let deleted = sqlx::query("DELETE FROM queued_items WHERE thread_id = ? AND id = ?")
            .bind(thread_id.to_string())
            .bind(item_id)
            .execute(&pool)
            .await?
            .rows_affected()
            > 0;
        pool.close().await;
        Ok(deleted)
    }
}
