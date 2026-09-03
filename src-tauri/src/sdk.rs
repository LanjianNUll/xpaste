use std::borrow::Cow;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use arboard::ImageData;
use base64::Engine;
use sqlx::SqlitePool;

use crate::history_repository::HistoryFilter;
use crate::models::{
    encode_png, ClipboardItem, ClipboardItemRow, HistoryPage, HistoryPageQuery, NewClipboardItem,
};
use crate::{classify, db, history_repository};

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 200;
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// 与 UI 无关的剪贴板 SDK，统一提供历史存储、查询、删除和写回能力。
#[derive(Clone)]
pub struct ClipboardSdk {
    pool: SqlitePool,
}

impl ClipboardSdk {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_history(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ClipboardItem>, String> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let rows = if query.trim().is_empty() {
            db::list_items(&self.pool, limit).await
        } else {
            db::search_items(&self.pool, query.trim(), limit).await
        }
        .map_err(database_error)?;

        Ok(rows.into_iter().map(ClipboardItem::from_summary).collect())
    }

    pub async fn list_history_by_date(
        &self,
        query: &str,
        start_ts: i64,
        end_ts: i64,
        limit: Option<i64>,
    ) -> Result<Vec<ClipboardItem>, String> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let rows = if query.trim().is_empty() {
            db::list_items_by_date_range(&self.pool, start_ts, end_ts, limit).await
        } else {
            db::search_items_by_date_range(&self.pool, query.trim(), start_ts, end_ts, limit).await
        }
        .map_err(database_error)?;

        Ok(rows.into_iter().map(ClipboardItem::from_summary).collect())
    }

    pub async fn history_page(&self, query: HistoryPageQuery) -> Result<HistoryPage, String> {
        let page_size = query.page_size.clamp(1, 100);
        let page = query.page.max(1);
        let offset = (page - 1).saturating_mul(page_size);
        let filter = HistoryFilter {
            start_ts: query.start_ts,
            end_ts: query.end_ts,
            keyword: &query.keyword,
            formats: &query.formats,
            categories: &query.categories,
        };
        let rows = history_repository::query_page(&self.pool, &filter, page_size, offset)
            .await
            .map_err(database_error)?;
        let total = history_repository::count(&self.pool, &filter)
            .await
            .map_err(database_error)?;
        Ok(HistoryPage {
            items: rows.into_iter().map(ClipboardItem::from_summary).collect(),
            total,
        })
    }

    pub async fn save_text(&self, text: String) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("文本内容不能为空".to_string());
        }
        let classification = classify::classify_text(trimmed);
        self.save_if_new(NewClipboardItem {
            format: classification.format,
            category: classification.category,
            text: Some(trimmed.to_string()),
            html: None,
            file_path: classification.file_path,
            color: classification.color,
            image: None,
            image_width: None,
            image_height: None,
            created_at: now_ms(),
        })
        .await
        .map(|_| ())
    }

    pub async fn save_png_base64(&self, image_base64: String) -> Result<(), String> {
        let encoded = image_base64
            .split_once(',')
            .map(|(_, data)| data)
            .unwrap_or(&image_base64);
        let png = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| "图片 Base64 数据无效".to_string())?;
        if png.len() > MAX_IMAGE_BYTES {
            return Err("图片超过 20MB 限制".to_string());
        }
        let mut reader = image::ImageReader::new(Cursor::new(png))
            .with_guessed_format()
            .map_err(|_| "图片格式不受支持".to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(MAX_IMAGE_BYTES as u64);
        reader.limits(limits);
        let rgba = reader
            .decode()
            .map_err(|_| "图片解码失败或占用内存过大".to_string())?
            .to_rgba8();
        let (width, height) = rgba.dimensions();
        let bytes = rgba.into_raw();
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("图片解码后超过 20MB 限制".to_string());
        }
        self.save_if_new(NewClipboardItem {
            format: "image".to_string(),
            category: "image".to_string(),
            text: None,
            html: None,
            file_path: None,
            color: None,
            image: Some(bytes),
            image_width: Some(i64::from(width)),
            image_height: Some(i64::from(height)),
            created_at: now_ms(),
        })
        .await
        .map(|_| ())
    }

    pub async fn save_if_new(&self, item: NewClipboardItem) -> Result<bool, String> {
        db::insert_or_increment_item(&self.pool, item)
            .await
            .map_err(database_error)
    }

    pub async fn image_base64(&self, id: i64, thumbnail: bool) -> Result<String, String> {
        let row = self.get_row(id).await?;
        if row.format != "image" {
            return Err("该记录不是图片".to_string());
        }
        let max_dimension = thumbnail.then_some(240);
        encode_png(row, max_dimension).ok_or_else(|| "图片数据损坏".to_string())
    }

    pub async fn copy_item(&self, id: i64) -> Result<(), String> {
        let row = self.get_row(id).await?;
        write_to_clipboard(row).map_err(|err| format!("写入剪贴板失败：{err}"))
    }

    pub async fn clear_history(&self) -> Result<(), String> {
        db::clear_all(&self.pool).await.map_err(database_error)
    }

    pub async fn delete_item(&self, id: i64) -> Result<(), String> {
        db::delete_item(&self.pool, id)
            .await
            .map_err(database_error)
    }

    pub async fn delete_items(&self, ids: &[i64]) -> Result<u64, String> {
        db::delete_items(&self.pool, ids)
            .await
            .map_err(database_error)
    }

    pub async fn delete_by_format(&self, format: &str) -> Result<u64, String> {
        db::delete_items_by_format(&self.pool, format)
            .await
            .map_err(database_error)
    }

    pub async fn delete_by_category(&self, category: &str) -> Result<u64, String> {
        db::delete_items_by_category(&self.pool, category)
            .await
            .map_err(database_error)
    }

    pub async fn delete_by_date(&self, start_ts: i64, end_ts: i64) -> Result<u64, String> {
        db::delete_items_by_date_range(&self.pool, start_ts, end_ts)
            .await
            .map_err(database_error)
    }

    pub async fn format_stats(&self) -> Result<Vec<(String, i64)>, String> {
        db::count_items_by_format(&self.pool)
            .await
            .map_err(database_error)
    }

    pub async fn category_stats(&self) -> Result<Vec<(String, i64)>, String> {
        db::count_items_by_category(&self.pool)
            .await
            .map_err(database_error)
    }

    async fn get_row(&self, id: i64) -> Result<ClipboardItemRow, String> {
        db::get_item(&self.pool, id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| "记录不存在".to_string())
    }
}

fn write_to_clipboard(row: ClipboardItemRow) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    match row.format.as_str() {
        "image" => {
            if let (Some(bytes), Some(width), Some(height)) =
                (row.image, row.image_width, row.image_height)
            {
                clipboard.set_image(ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: Cow::Owned(bytes),
                })?;
            }
        }
        "html" => {
            if let Some(html) = row.html {
                clipboard.set_html(html, row.text)?;
            }
        }
        _ => {
            if let Some(text) = row.text.or(row.file_path).or(row.color) {
                clipboard.set_text(text)?;
            }
        }
    }
    Ok(())
}

fn database_error(error: sqlx::Error) -> String {
    format!("数据库操作失败：{error}")
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
