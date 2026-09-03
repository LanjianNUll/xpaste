use std::path::Path;

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
    SqlitePool,
};

use crate::models::{
    content_fingerprint, ClipboardItemRow, ClipboardItemSummaryRow, NewClipboardItem,
};

#[derive(sqlx::FromRow)]
struct ClipboardMigrationRow {
    id: i64,
    format: String,
    category: String,
    text: Option<String>,
    html: Option<String>,
    file_path: Option<String>,
    color: Option<String>,
    image: Option<Vec<u8>>,
    image_width: Option<i64>,
    image_height: Option<i64>,
    created_at: i64,
}

pub async fn init_db(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .pragma("temp_store", "FILE")
        .pragma("cache_size", "-2048")
        .pragma("wal_autocheckpoint", "500");

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        // 桌面应用以低资源占用优先，避免连接池和 SQLite 页缓存无谓增长。
        .max_connections(2)
        .connect_with(options)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS clipboard_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      format TEXT NOT NULL,
      category TEXT NOT NULL,
      text TEXT,
      html TEXT,
      file_path TEXT,
      color TEXT,
      image BLOB,
      image_width INTEGER,
      image_height INTEGER,
      created_at INTEGER NOT NULL,
      copy_count INTEGER NOT NULL DEFAULT 1,
      content_hash TEXT
    )",
    )
    .execute(&pool)
    .await?;
    migrate_duplicate_records(&pool).await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_items_created_at
         ON clipboard_items(created_at DESC)",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_items_content_hash
         ON clipboard_items(content_hash)",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_items_format_created_at
         ON clipboard_items(format, created_at DESC)",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_items_category_created_at
         ON clipboard_items(category, created_at DESC)",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

/// 首次升级时合并历史重复内容，并为后续记录补充去重字段。
async fn migrate_duplicate_records(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let columns = sqlx::query_as::<_, (i64, String, String, i64, Option<String>, i64)>(
        "PRAGMA table_info(clipboard_items)",
    )
    .fetch_all(pool)
    .await?;
    let has_copy_count = columns.iter().any(|column| column.1 == "copy_count");
    let has_content_hash = columns.iter().any(|column| column.1 == "content_hash");
    if !has_copy_count {
        sqlx::query("ALTER TABLE clipboard_items ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 1")
            .execute(pool)
            .await?;
    }
    if !has_content_hash {
        sqlx::query("ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT")
            .execute(pool)
            .await?;
    }
    let pending_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM clipboard_items WHERE content_hash IS NULL",
    )
    .fetch_one(pool)
    .await?;
    if pending_count == 0 {
        return Ok(());
    }
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "CREATE TEMP TABLE clipboard_duplicate_groups AS
         SELECT MAX(id) AS keep_id, SUM(copy_count) AS total_count
         FROM clipboard_items
         GROUP BY format, category, text, html, file_path, color, image, image_width, image_height",
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE clipboard_items
         SET copy_count = (
           SELECT total_count FROM clipboard_duplicate_groups WHERE keep_id = clipboard_items.id
         )
         WHERE id IN (SELECT keep_id FROM clipboard_duplicate_groups)",
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "DELETE FROM clipboard_items
         WHERE id NOT IN (SELECT keep_id FROM clipboard_duplicate_groups)",
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query("DROP TABLE clipboard_duplicate_groups")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    loop {
        let rows = sqlx::query_as::<_, ClipboardMigrationRow>(
            "SELECT id, format, category, text, html, file_path, color, image,
                    image_width, image_height, created_at
             FROM clipboard_items WHERE content_hash IS NULL LIMIT 10",
        )
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let id = row.id;
            let item = NewClipboardItem {
                format: row.format,
                category: row.category,
                text: row.text,
                html: row.html,
                file_path: row.file_path,
                color: row.color,
                image: row.image,
                image_width: row.image_width,
                image_height: row.image_height,
                created_at: row.created_at,
            };
            sqlx::query("UPDATE clipboard_items SET content_hash = ? WHERE id = ?")
                .bind(content_fingerprint(&item))
                .bind(id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// 插入新内容；已有相同指纹时只更新时间并累加复制次数。
pub async fn insert_or_increment_item(
    pool: &SqlitePool,
    item: NewClipboardItem,
) -> Result<bool, sqlx::Error> {
    let content_hash = content_fingerprint(&item);
    let copy_count = sqlx::query_scalar::<_, i64>(
        "INSERT INTO clipboard_items (
        format, category, text, html, file_path, color, image, image_width, image_height,
        created_at, copy_count, content_hash
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
      ON CONFLICT(content_hash) DO UPDATE SET
        created_at = excluded.created_at,
        copy_count = clipboard_items.copy_count + 1
      RETURNING copy_count",
    )
    .bind(item.format)
    .bind(item.category)
    .bind(item.text)
    .bind(item.html)
    .bind(item.file_path)
    .bind(item.color)
    .bind(item.image)
    .bind(item.image_width)
    .bind(item.image_height)
    .bind(item.created_at)
    .bind(content_hash)
    .fetch_one(pool)
    .await?;

    Ok(copy_count == 1)
}

pub async fn list_items(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<ClipboardItemSummaryRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ClipboardItemSummaryRow>(
        "SELECT id, format, category, substr(text, 1, 4096) AS text,
            substr(html, 1, 4096) AS html, file_path, color, image_width, image_height, created_at,
            copy_count
     FROM clipboard_items
     ORDER BY created_at DESC
     LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn search_items(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> Result<Vec<ClipboardItemSummaryRow>, sqlx::Error> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<_, ClipboardItemSummaryRow>(
        "SELECT id, format, category, substr(text, 1, 4096) AS text,
            substr(html, 1, 4096) AS html, file_path, color, image_width, image_height, created_at,
            copy_count
     FROM clipboard_items
     WHERE text LIKE ? OR html LIKE ? OR file_path LIKE ? OR color LIKE ?
     ORDER BY created_at DESC
     LIMIT ?",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_item(pool: &SqlitePool, id: i64) -> Result<Option<ClipboardItemRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, ClipboardItemRow>(
        "SELECT format, text, html, file_path, color, image, image_width, image_height
     FROM clipboard_items
     WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn list_items_by_date_range(
    pool: &SqlitePool,
    start_ts: i64,
    end_ts: i64,
    limit: i64,
) -> Result<Vec<ClipboardItemSummaryRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ClipboardItemSummaryRow>(
        "SELECT id, format, category, substr(text, 1, 4096) AS text,
            substr(html, 1, 4096) AS html, file_path, color, image_width, image_height, created_at,
            copy_count
     FROM clipboard_items
     WHERE created_at >= ? AND created_at <= ?
     ORDER BY created_at DESC
     LIMIT ?",
    )
    .bind(start_ts)
    .bind(end_ts)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn search_items_by_date_range(
    pool: &SqlitePool,
    query: &str,
    start_ts: i64,
    end_ts: i64,
    limit: i64,
) -> Result<Vec<ClipboardItemSummaryRow>, sqlx::Error> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<_, ClipboardItemSummaryRow>(
        "SELECT id, format, category, substr(text, 1, 4096) AS text,
            substr(html, 1, 4096) AS html, file_path, color, image_width, image_height, created_at,
            copy_count
     FROM clipboard_items
     WHERE (text LIKE ? OR html LIKE ? OR file_path LIKE ? OR color LIKE ?)
       AND created_at >= ? AND created_at <= ?
     ORDER BY created_at DESC
     LIMIT ?",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(start_ts)
    .bind(end_ts)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn clear_all(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM clipboard_items")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_item(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM clipboard_items WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_items(pool: &SqlitePool, ids: &[i64]) -> Result<u64, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let mut deleted = 0;
    for id in ids {
        deleted += sqlx::query("DELETE FROM clipboard_items WHERE id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    }
    transaction.commit().await?;
    Ok(deleted)
}

pub async fn delete_items_by_format(pool: &SqlitePool, format: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM clipboard_items WHERE format = ?")
        .bind(format)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn delete_items_by_category(
    pool: &SqlitePool,
    category: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM clipboard_items WHERE category = ?")
        .bind(category)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn delete_items_by_date_range(
    pool: &SqlitePool,
    start_ts: i64,
    end_ts: i64,
) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM clipboard_items WHERE created_at >= ? AND created_at <= ?")
            .bind(start_ts)
            .bind(end_ts)
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

pub async fn count_items_by_format(pool: &SqlitePool) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT format, COUNT(*) FROM clipboard_items GROUP BY format",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_items_by_category(pool: &SqlitePool) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT category, COUNT(*) FROM clipboard_items GROUP BY category",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
