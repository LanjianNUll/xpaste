use std::borrow::Cow;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use arboard::ImageData;
use base64::Engine;
use sqlx::SqlitePool;

use crate::history_repository::HistoryFilter;
use crate::models::{
    encode_png, ClipboardItem, ClipboardItemRow, HistoryPage, HistoryPageQuery, NewClipboardItem,
    SyncExportItem, SyncImportItem,
};
use crate::{classify, db, history_repository};

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 200;
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
// 收藏是用户精选的集合，主面板要能一页看全，默认限额比历史更宽。
const FAVORITE_DEFAULT_LIMIT: i64 = 200;
const FAVORITE_MAX_LIMIT: i64 = 1000;
/// 单次同步最多搬运的记录条数，控制单个请求的体积。
const SYNC_BATCH: i64 = 200;
/// 单条同步记录允许的最大文本体积，超过直接丢弃而不是截断（截断会悄悄改内容）。
const SYNC_MAX_TEXT_BYTES: usize = 512 * 1024;

/// 条目数据来源：历史记录或收藏快照，决定按哪个表读写完整内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemSource {
    History,
    Favorite,
}

impl ItemSource {
    /// 前端命令参数解析：未传或无法识别时回落到历史记录。
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("favorite") => ItemSource::Favorite,
            _ => ItemSource::History,
        }
    }
}

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

    pub async fn image_base64(
        &self,
        source: ItemSource,
        id: i64,
        thumbnail: bool,
    ) -> Result<String, String> {
        let row = self.get_row(source, id).await?;
        if row.format != "image" {
            return Err("该记录不是图片".to_string());
        }
        let max_dimension = thumbnail.then_some(240);
        encode_png(row, max_dimension).ok_or_else(|| "图片数据损坏".to_string())
    }

    pub async fn copy_item(&self, source: ItemSource, id: i64) -> Result<(), String> {
        let row = self.get_row(source, id).await?;
        write_to_clipboard(row).map_err(|err| format!("写入剪贴板失败：{err}"))
    }

    /// 取一条记录的完整文本，供编辑器插入素材使用。
    ///
    /// 列表查询为了控制传输量会把 text/html 截到 4096 字符（见 db.rs 的
    /// `substr(text, 1, 4096)`）。编辑器是唯一需要原文的地方，因此这里按 id
    /// 重新读整行，避免把长文静默插成半截。
    pub async fn item_text(&self, source: ItemSource, id: i64) -> Result<String, String> {
        let row = self.get_row(source, id).await?;
        Ok(row
            .text
            .or(row.html)
            .or(row.file_path)
            .or(row.color)
            .unwrap_or_default())
    }

    /// 收藏切换：已收藏则取消（返回 false），否则把历史条目整行快照进收藏表（返回 true）。
    pub async fn toggle_favorite(&self, id: i64) -> Result<bool, String> {
        let (content_hash, created_at) = db::get_item_favorite_source(&self.pool, id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| "记录不存在".to_string())?;
        if db::delete_favorite_by_hash(&self.pool, &content_hash)
            .await
            .map_err(database_error)?
        {
            return Ok(false);
        }
        let row = self.get_row(ItemSource::History, id).await?;
        db::insert_favorite(&self.pool, &row, &content_hash, created_at, now_ms())
            .await
            .map_err(database_error)?;
        Ok(true)
    }

    pub async fn remove_favorite(&self, id: i64) -> Result<(), String> {
        db::delete_favorite(&self.pool, id)
            .await
            .map_err(database_error)
    }

    pub async fn list_favorites(
        &self,
        query: &str,
        format: Option<String>,
        limit: Option<i64>,
    ) -> Result<Vec<ClipboardItem>, String> {
        let limit = limit
            .unwrap_or(FAVORITE_DEFAULT_LIMIT)
            .clamp(1, FAVORITE_MAX_LIMIT);
        let format = format.filter(|value| !value.trim().is_empty());
        let rows = if query.trim().is_empty() {
            db::list_favorites(&self.pool, format.as_deref(), limit).await
        } else {
            db::search_favorites(&self.pool, query.trim(), format.as_deref(), limit).await
        }
        .map_err(database_error)?;

        Ok(rows.into_iter().map(ClipboardItem::from_favorite).collect())
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

    /// 多设备同步：取出 `since_ts` 之后新增的记录（时间正序）。
    ///
    /// 单次上限 200 条，避免一次把整个历史塞进一个请求。
    pub async fn sync_export(
        &self,
        since_ts: i64,
        limit: Option<i64>,
    ) -> Result<Vec<SyncExportItem>, String> {
        let limit = limit.unwrap_or(SYNC_BATCH).clamp(1, SYNC_BATCH);
        db::sync_export_items(&self.pool, since_ts, limit)
            .await
            .map_err(database_error)
    }

    /// 多设备同步：把对端记录写进本地库，返回真正新增的条数。
    pub async fn sync_import(&self, items: Vec<SyncImportItem>) -> Result<u64, String> {
        let mut inserted = 0_u64;
        for raw in items {
            let Some(item) = sanitize_import(raw) else {
                continue;
            };
            if db::sync_insert_remote(&self.pool, &item)
                .await
                .map_err(database_error)?
            {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    async fn get_row(&self, source: ItemSource, id: i64) -> Result<ClipboardItemRow, String> {        match source {
            ItemSource::History => db::get_item(&self.pool, id).await,
            ItemSource::Favorite => db::get_favorite_row(&self.pool, id).await,
        }
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
        // 文件/文件夹记录写回的就是完整路径：多选时一行一个路径。
        "file" => {
            if let Some(text) = row.text.or(row.file_path) {
                clipboard.set_text(text)?;
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

/// 同步入库前的净化。
///
/// 记录虽然是从加密房间里解出来的，但内容仍然来自"另一台设备"，而房间是
/// 凭同步码共享的——这个边界一律按不可信输入处理：长度封顶、格式收敛、
/// 时间戳修正、空记录丢弃。宁可少同步一条，也不能让脏数据把界面撑爆。
fn sanitize_import(mut item: SyncImportItem) -> Option<SyncImportItem> {
    item.format = item.format.trim().to_ascii_lowercase();
    item.category = item.category.trim().to_ascii_lowercase();

    // 图片不做第一版同步，收到也丢弃，避免对端版本不一致时写进空图片记录。
    if item.format == "image" || item.category == "image" {
        return None;
    }
    if item.format.is_empty() {
        item.format = "text".to_string();
    }
    if item.category.is_empty() {
        item.category = item.format.clone();
    }

    let has_content = item
        .text
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        || item.html.is_some()
        || item.color.is_some()
        || item.file_path.is_some();
    if !has_content {
        return None;
    }

    let total = [item.text.as_ref(), item.html.as_ref(), item.file_path.as_ref()]
        .into_iter()
        .flatten()
        .map(String::len)
        .sum::<usize>();
    if total > SYNC_MAX_TEXT_BYTES {
        return None;
    }

    // 对端时钟可能不准：未来时间会让记录一直钉在列表顶部，归零则永远排在末尾。
    let now = now_ms();
    if item.created_at <= 0 || item.created_at > now + 86_400_000 {
        item.created_at = now;
    }

    Some(item)
}
