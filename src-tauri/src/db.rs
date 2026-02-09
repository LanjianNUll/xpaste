use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

use crate::models::{ClipboardItemRow, NewClipboardItem};

pub async fn init_db(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
  let options = SqliteConnectOptions::new()
    .filename(db_path)
    .create_if_missing(true);

  let pool = sqlx::sqlite::SqlitePoolOptions::new()
    .max_connections(5)
    .connect_with(options)
    .await?;

  sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;
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
      created_at INTEGER NOT NULL
    )",
  )
  .execute(&pool)
  .await?;

  Ok(pool)
}

pub async fn insert_item(pool: &SqlitePool, item: NewClipboardItem) -> Result<(), sqlx::Error> {
  sqlx::query(
    "INSERT INTO clipboard_items (
        format, category, text, html, file_path, color, image, image_width, image_height, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
  .execute(pool)
  .await?;

  Ok(())
}

pub async fn list_items(pool: &SqlitePool, limit: i64) -> Result<Vec<ClipboardItemRow>, sqlx::Error> {
  let rows = sqlx::query_as::<_, ClipboardItemRow>(
    "SELECT id, format, category, text, html, file_path, color, image, image_width, image_height, created_at
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
) -> Result<Vec<ClipboardItemRow>, sqlx::Error> {
  let pattern = format!("%{}%", query);
  let rows = sqlx::query_as::<_, ClipboardItemRow>(
    "SELECT id, format, category, text, html, file_path, color, image, image_width, image_height, created_at
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
    "SELECT id, format, category, text, html, file_path, color, image, image_width, image_height, created_at
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
) -> Result<Vec<ClipboardItemRow>, sqlx::Error> {
  let rows = sqlx::query_as::<_, ClipboardItemRow>(
    "SELECT id, format, category, text, html, file_path, color, image, image_width, image_height, created_at
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
) -> Result<Vec<ClipboardItemRow>, sqlx::Error> {
  let pattern = format!("%{}%", query);
  let rows = sqlx::query_as::<_, ClipboardItemRow>(
    "SELECT id, format, category, text, html, file_path, color, image, image_width, image_height, created_at
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
