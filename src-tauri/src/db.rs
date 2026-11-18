use std::path::Path;

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
    SqlitePool,
};

use crate::models::{
    content_fingerprint, encode_png_data, ClipboardItemRow, ClipboardItemSummaryRow,
    FavoriteSummaryRow, NewClipboardItem, SyncExportItem, SyncExportRow, SyncImportItem,
};

/// 同步图片先压到 1280px 再编码：肉眼够用，体积又能压到中继可接受的范围。
const SYNC_IMAGE_MAX_DIMENSION: u32 = 1280;
/// 单张图片的 Base64 上限：超过就只同步文字，避免一次请求被中继直接拒绝。
const SYNC_IMAGE_MAX_BASE64_BYTES: usize = 3 * 1024 * 1024;

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
        // 启动时的图片压缩/清理会短暂持有写锁，给其它连接多留一点等待时间
        .busy_timeout(std::time::Duration::from_secs(10))
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
      content_hash TEXT,
      source_device TEXT
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

    // 收藏是独立于历史的快照表：内容整行拷贝，删除/清空历史不影响收藏。
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS favorites (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      content_hash TEXT NOT NULL,
      format TEXT NOT NULL,
      category TEXT NOT NULL,
      text TEXT,
      html TEXT,
      file_path TEXT,
      color TEXT,
      image BLOB,
      image_width INTEGER,
      image_height INTEGER,
      copy_count INTEGER NOT NULL DEFAULT 1,
      created_at INTEGER NOT NULL,
      favorited_at INTEGER NOT NULL,
      source_device TEXT
    )",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_favorites_content_hash
         ON favorites(content_hash)",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_favorites_favorited_at
         ON favorites(favorited_at DESC)",
    )
    .execute(&pool)
    .await?;

    // 必须等两张表都建好再补列：全新库上对不存在的表 ALTER 会直接报错
    migrate_source_device(&pool).await?;

    Ok(pool)
}

/// 给老库补上「来源设备」列。
///
/// 新库建表时已经带上该列，这里只处理升级上来的库：逐表查一次 PRAGMA，
/// 缺了才 ALTER，重复执行没有副作用。
async fn migrate_source_device(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for table in ["clipboard_items", "favorites"] {
        let columns = sqlx::query_as::<_, (i64, String, String, i64, Option<String>, i64)>(
            &format!("PRAGMA table_info({table})"),
        )
        .fetch_all(pool)
        .await?;
        if columns.iter().any(|column| column.1 == "source_device") {
            continue;
        }
        sqlx::query(&format!("ALTER TABLE {table} ADD COLUMN source_device TEXT"))
            .execute(pool)
            .await?;
    }
    Ok(())
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
            copy_count, source_device,
            EXISTS(SELECT 1 FROM favorites f WHERE f.content_hash = clipboard_items.content_hash)
              AS favorited
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
            copy_count, source_device,
            EXISTS(SELECT 1 FROM favorites f WHERE f.content_hash = clipboard_items.content_hash)
              AS favorited
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
        "SELECT format, category, text, html, file_path, color, image, image_width, image_height
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
            copy_count, source_device,
            EXISTS(SELECT 1 FROM favorites f WHERE f.content_hash = clipboard_items.content_hash)
              AS favorited
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
            copy_count, source_device,
            EXISTS(SELECT 1 FROM favorites f WHERE f.content_hash = clipboard_items.content_hash)
              AS favorited
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

/// 收藏内容快照：整行拷入 favorites，已存在相同指纹时静默跳过（幂等）。
pub async fn insert_favorite(
    pool: &SqlitePool,
    row: &ClipboardItemRow,
    content_hash: &str,
    created_at: i64,
    favorited_at: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO favorites (
        content_hash, format, category, text, html, file_path, color, image,
        image_width, image_height, copy_count, created_at, favorited_at, source_device
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?,
        (SELECT source_device FROM clipboard_items WHERE content_hash = ? LIMIT 1))",
    )
    .bind(content_hash)
    .bind(&row.format)
    .bind(&row.category)
    .bind(&row.text)
    .bind(&row.html)
    .bind(&row.file_path)
    .bind(&row.color)
    .bind(&row.image)
    .bind(row.image_width)
    .bind(row.image_height)
    .bind(created_at)
    .bind(favorited_at)
    .bind(content_hash)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_favorite_by_hash(pool: &SqlitePool, content_hash: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM favorites WHERE content_hash = ?")
        .bind(content_hash)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_favorite(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM favorites WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 按历史记录 id 读取内容指纹与原始复制时间，用于收藏快照。
pub async fn get_item_favorite_source(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT content_hash, created_at FROM clipboard_items WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(hash, created_at)| hash.map(|hash| (hash, created_at))))
}

/// 读取收藏快照的完整行，形状与历史行一致，写回剪贴板/图片编码可直接复用。
pub async fn get_favorite_row(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<ClipboardItemRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, ClipboardItemRow>(
        "SELECT format, category, text, html, file_path, color, image, image_width, image_height
     FROM favorites
     WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn list_favorites(
    pool: &SqlitePool,
    format: Option<&str>,
    limit: i64,
) -> Result<Vec<FavoriteSummaryRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, FavoriteSummaryRow>(
        "SELECT id, format, category, substr(text, 1, 4096) AS text,
            substr(html, 1, 4096) AS html, file_path, color, image_width, image_height,
            copy_count, favorited_at, source_device
     FROM favorites
     WHERE (? IS NULL OR format = ?)
     ORDER BY favorited_at DESC
     LIMIT ?",
    )
    .bind(format)
    .bind(format)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn search_favorites(
    pool: &SqlitePool,
    query: &str,
    format: Option<&str>,
    limit: i64,
) -> Result<Vec<FavoriteSummaryRow>, sqlx::Error> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query_as::<_, FavoriteSummaryRow>(
        "SELECT id, format, category, substr(text, 1, 4096) AS text,
            substr(html, 1, 4096) AS html, file_path, color, image_width, image_height,
            copy_count, favorited_at, source_device
     FROM favorites
     WHERE (? IS NULL OR format = ?)
       AND (text LIKE ? OR html LIKE ? OR file_path LIKE ? OR color LIKE ?)
     ORDER BY favorited_at DESC
     LIMIT ?",
    )
    .bind(format)
    .bind(format)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// 导出 `since_ts` 之后新增的记录，按时间正序——同步要保证对端按发生顺序落库。
///
/// 这里不做 `substr(...)` 截断：列表接口为了省流量会截到 4096 字符，
/// 但同步必须搬运完整内容，否则对端拿到的是半截文本。
pub async fn sync_export_items(
    pool: &SqlitePool,
    since_ts: i64,
    limit: i64,
    include_images: bool,
) -> Result<Vec<SyncExportItem>, sqlx::Error> {
    // 不开图片同步时连 BLOB 都不读，避免一次导出把几十 MB 图片搬进内存
    let rows = if include_images {
        sqlx::query_as::<_, SyncExportRow>(
            "SELECT id, format, category, text, html, color, file_path, created_at, copy_count,
                    image, image_width, image_height
             FROM clipboard_items
             WHERE created_at > ?
             ORDER BY created_at ASC, id ASC
             LIMIT ?",
        )
        .bind(since_ts)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, SyncExportRow>(
            "SELECT id, format, category, text, html, color, file_path, created_at, copy_count,
                    NULL AS image, NULL AS image_width, NULL AS image_height
             FROM clipboard_items
             WHERE created_at > ?
             ORDER BY created_at ASC, id ASC
             LIMIT ?",
        )
        .bind(since_ts)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|row| {
            row.into_export_item(SYNC_IMAGE_MAX_DIMENSION, SYNC_IMAGE_MAX_BASE64_BYTES)
        })
        .collect())
}

/// 写入对端推来的记录。
///
/// 用 `INSERT OR IGNORE` 撞 content_hash 唯一索引，只保留"新内容"。
/// 刻意不累加 copy_count：对端的一条记录在本机第一次出现时复制次数应当是 1，
/// 而不是把它当成"又复制了一次"，否则两台设备互相同步会把计数越推越高。
/// 来源设备名做一次规整：去空白、限长、空串当作没有来源。
fn device_of(item: &SyncImportItem) -> Option<String> {
    item.device
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(48).collect())
}

pub async fn sync_insert_remote(
    pool: &SqlitePool,
    item: &SyncImportItem,
    image: Option<(Vec<u8>, i64, i64)>,
) -> Result<bool, sqlx::Error> {
    let new_item = NewClipboardItem {
        format: item.format.clone(),
        category: item.category.clone(),
        text: item.text.clone(),
        html: item.html.clone(),
        file_path: item.file_path.clone(),
        color: item.color.clone(),
        image: image.as_ref().map(|(bytes, _, _)| bytes.clone()),
        image_width: image.as_ref().map(|(_, width, _)| *width),
        image_height: image.as_ref().map(|(_, _, height)| *height),
        created_at: item.created_at,
    };
    let content_hash = content_fingerprint(&new_item);
    let copy_count = item.copy_count.unwrap_or(1).clamp(1, 1_000_000);

    let result = sqlx::query(
        "INSERT OR IGNORE INTO clipboard_items (
           format, category, text, html, file_path, color, image,
           image_width, image_height, created_at, copy_count, content_hash, source_device
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&new_item.format)
    .bind(&new_item.category)
    .bind(&new_item.text)
    .bind(&new_item.html)
    .bind(&new_item.file_path)
    .bind(&new_item.color)
    .bind(&new_item.image)
    .bind(new_item.image_width)
    .bind(new_item.image_height)
    .bind(new_item.created_at)
    .bind(copy_count)
    .bind(&content_hash)
    .bind(device_of(item))
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

// ── 图片存储优化 ──────────────────────────────────────────────

/// 历史图片的总预算。超过就从最老的图片开始删；收藏是用户主动留的，不在此列。
const IMAGE_BUDGET_BYTES: i64 = 300 * 1024 * 1024;
/// 旧图迁移时允许的最长边：超过就等比缩小，避免把超大截图继续放大存储。
const IMAGE_MIGRATE_MAX_DIMENSION: u32 = 2560;
/// PNG 文件头，用来判断一行图片是不是已经是压缩格式。
const PNG_MAGIC_SQL: &str = "x'89504E470D0A1A0A'";

/// 把历史图片控制到预算以内，返回删除的条数。
///
/// `SUM(length(image))` 只读 BLOB 长度，不搬内容，几百上千条也很快。
pub async fn enforce_image_budget(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let mut deleted = 0_u64;
    loop {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(image)), 0) FROM clipboard_items WHERE image IS NOT NULL",
        )
        .fetch_one(pool)
        .await?;
        if total <= IMAGE_BUDGET_BYTES {
            break;
        }

        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM clipboard_items
             WHERE image IS NOT NULL
             ORDER BY created_at ASC
             LIMIT 5",
        )
        .fetch_all(pool)
        .await?;
        if ids.is_empty() {
            break;
        }
        for id in ids {
            sqlx::query("DELETE FROM clipboard_items WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

#[derive(Debug, Clone)]
pub struct ImageStorageReport {
    pub converted: u64,
    pub deleted: u64,
    pub size_before: u64,
    pub size_after: u64,
}

/// 一次性整理图片存储：旧 RGBA 行压成 PNG，超预算的历史图片淘汰，最后 VACUUM 收缩文件。
///
/// 在应用启动后由后台任务调用，不阻塞窗口出现；读取方同时兼容新旧两种格式，
/// 所以压缩过程中界面仍能正常展示图片。
pub async fn optimize_image_storage(
    pool: &SqlitePool,
    db_path: &Path,
) -> Result<ImageStorageReport, sqlx::Error> {
    let size_before = std::fs::metadata(db_path).map(|meta| meta.len()).unwrap_or(0);
    let converted = compress_legacy_images(pool).await?;
    let deleted = enforce_image_budget(pool).await?;

    let free_pages: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(pool)
        .await?;
    if converted > 0 || deleted > 0 || free_pages > 64 {
        // VACUUM 需要独占：刚启动时通常没有别的读写，失败就留到下次启动再试
        for attempt in 0..3 {
            match sqlx::query("VACUUM").execute(pool).await {
                Ok(_) => break,
                Err(error) if attempt < 2 => {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if attempt == 1 {
                        eprintln!("vacuum retry failed: {error}");
                    }
                }
                Err(_) => break,
            }
        }
    }

    // VACUUM 会写大量页，WAL 可能膨胀到上百 MB；截断一次，
    // 否则下次启动 SQLite 还要恢复这么长的日志，表现为打开很慢。
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_optional(pool)
        .await;
    let _ = sqlx::query("PRAGMA optimize").execute(pool).await;

    let size_after = std::fs::metadata(db_path).map(|meta| meta.len()).unwrap_or(0);
    Ok(ImageStorageReport {
        converted,
        deleted,
        size_before,
        size_after,
    })
}

/// 把旧库里按 RGBA 存的图片逐批压成 PNG，返回转换条数。
async fn compress_legacy_images(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let mut converted = 0_u64;
    for table in ["clipboard_items", "favorites"] {
        loop {
            // 已经在 PNG 里的行直接跳过；SQLite 侧先过滤，避免把大 BLOB 全读出来
            let rows = sqlx::query_as::<_, (i64, Vec<u8>, Option<i64>, Option<i64>)>(&format!(
                "SELECT id, image, image_width, image_height FROM {table}
                 WHERE image IS NOT NULL
                   AND substr(image, 1, 8) <> {PNG_MAGIC_SQL}
                 LIMIT 8"
            ))
            .fetch_all(pool)
            .await?;
            if rows.is_empty() {
                break;
            }

            for (id, bytes, width, height) in rows {
                let encoded = width
                    .zip(height)
                    .and_then(|(width, height)| {
                        encode_png_data(
                            bytes,
                            width,
                            height,
                            Some(IMAGE_MIGRATE_MAX_DIMENSION),
                        )
                    });

                match encoded {
                    Some((png, width, height)) => {
                        sqlx::query(&format!(
                            "UPDATE {table} SET image = ?, image_width = ?, image_height = ? WHERE id = ?"
                        ))
                        .bind(png)
                        .bind(width)
                        .bind(height)
                        .bind(id)
                        .execute(pool)
                        .await?;
                        converted += 1;
                    }
                    None => {
                        // 尺寸对不上的损坏记录：清掉 BLOB，别让迁移反复卡在同一行
                        sqlx::query(&format!("UPDATE {table} SET image = NULL WHERE id = ?"))
                            .bind(id)
                            .execute(pool)
                            .await?;
                    }
                }
            }
            // 一批压缩完让出 CPU：大库迁移时不要长时间占满一个核心，避免整机发卡
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        }
    }
    Ok(converted)
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use crate::models::is_png_blob;

    /// 旧 RGBA 图片应该在启动优化里被压成 PNG，且读取路径仍然可用。
    #[test]
    fn image_storage_migration() {
        tauri::async_runtime::block_on(async {
            let dir = std::env::temp_dir().join(format!("xpaste-db-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("创建临时目录");
            let path = dir.join("clipboard.db");
            let pool = init_db(&path).await.expect("初始化数据库");

            let raw = vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ];
            insert_or_increment_item(
                &pool,
                NewClipboardItem {
                    format: "image".to_string(),
                    category: "image".to_string(),
                    text: None,
                    html: None,
                    file_path: None,
                    color: None,
                    image: Some(raw),
                    image_width: Some(2),
                    image_height: Some(2),
                    created_at: 1,
                },
            )
            .await
            .expect("写入旧格式图片");

            let report = optimize_image_storage(&pool, &path)
                .await
                .expect("迁移图片存储");
            assert_eq!(report.converted, 1, "应转换一张旧图");

            let blob: Vec<u8> = sqlx::query_scalar("SELECT image FROM clipboard_items LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("读取迁移后的图片");
            assert!(is_png_blob(&blob), "迁移后必须是 PNG");

            pool.close().await;
            let _ = std::fs::remove_dir_all(&dir);
        });
    }
}
