use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Message {
    pub id: String,
    pub to_addr: String,
    pub payload: String,
    pub status: String,
    pub attempts: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_retry_at: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct InboxMessage {
    pub id: i64,
    pub from_addr: String,
    pub payload: String,
    pub received_at: i64,
}

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(db_path)?
            .create_if_missing(true);
        
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        
        Ok(Database { pool })
    }
    
    pub async fn initialize(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                to_addr TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                next_retry_at INTEGER
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS inbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_addr TEXT NOT NULL,
                payload TEXT NOT NULL,
                received_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_messages_status 
            ON messages(status, next_retry_at)
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn insert_message(
        &self,
        to_addr: &str,
        payload: &str,
    ) -> Result<String, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        
        sqlx::query(
            r#"
            INSERT INTO messages (id, to_addr, payload, status, attempts, created_at, updated_at)
            VALUES (?, ?, ?, 'queued', 0, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(to_addr)
        .bind(payload)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(id)
    }
    
    pub async fn get_message(&self, id: &str) -> Result<Option<Message>, sqlx::Error> {
        let msg = sqlx::query_as::<_, Message>(
            r#"
            SELECT id, to_addr, payload, status, attempts, created_at, updated_at, next_retry_at
            FROM messages
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(msg)
    }
    
    pub async fn update_message_status(
        &self,
        id: &str,
        status: &str,
        attempts: i32,
        next_retry_at: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        
        sqlx::query(
            r#"
            UPDATE messages
            SET status = ?, attempts = ?, updated_at = ?, next_retry_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(attempts)
        .bind(now)
        .bind(next_retry_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_pending_messages(&self) -> Result<Vec<Message>, sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        
        let messages = sqlx::query_as::<_, Message>(
            r#"
            SELECT id, to_addr, payload, status, attempts, created_at, updated_at, next_retry_at
            FROM messages
            WHERE status = 'queued'
            AND (next_retry_at IS NULL OR next_retry_at <= ?)
            ORDER BY created_at ASC
            LIMIT 100
            "#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(messages)
    }
    
    pub async fn insert_inbox_message(
        &self,
        from_addr: &str,
        payload: &str,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        
        let result = sqlx::query(
            r#"
            INSERT INTO inbox (from_addr, payload, received_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(from_addr)
        .bind(payload)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(result.last_insert_rowid())
    }
    
    pub async fn get_inbox_messages(&self, limit: i64) -> Result<Vec<InboxMessage>, sqlx::Error> {
        let messages = sqlx::query_as::<_, InboxMessage>(
            r#"
            SELECT id, from_addr, payload, received_at
            FROM inbox
            ORDER BY received_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(messages)
    }
}
