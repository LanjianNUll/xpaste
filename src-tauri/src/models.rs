use std::io::Cursor;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ClipboardItemRow {
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub file_path: Option<String>,
    pub color: Option<String>,
    pub image: Option<Vec<u8>>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ClipboardItemSummaryRow {
    pub id: i64,
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub file_path: Option<String>,
    pub color: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub created_at: i64,
    pub copy_count: i64,
    pub favorited: bool,
    /// 同步进来的记录会带上来源设备名；本机复制的内容为 None
    pub source_device: Option<String>,
}

/// 收藏列表行：favorited_at 映射到 ClipboardItem.created_at 用于展示。
#[derive(Debug, Clone, FromRow)]
pub struct FavoriteSummaryRow {
    pub id: i64,
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub file_path: Option<String>,
    pub color: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub copy_count: i64,
    pub favorited_at: i64,
    /// 收藏时一并带过来的来源设备名，本机内容为 None
    pub source_device: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: i64,
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub file_path: Option<String>,
    pub color: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub created_at: i64,
    pub copy_count: i64,
    pub favorited: bool,
    pub source_device: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<ClipboardItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageQuery {    /// 缺省视为不限时间：快捷面板的全局搜索就是靠不传这两个字段实现的。
    #[serde(default)]
    pub start_ts: Option<i64>,
    #[serde(default)]
    pub end_ts: Option<i64>,
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct NewClipboardItem {
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub file_path: Option<String>,
    pub color: Option<String>,
    pub image: Option<Vec<u8>>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub created_at: i64,
}

impl ClipboardItem {
    pub fn from_summary(row: ClipboardItemSummaryRow) -> Self {
        Self {
            id: row.id,
            format: row.format,
            category: row.category,
            text: row.text,
            html: row.html,
            file_path: row.file_path,
            color: row.color,
            image_width: row.image_width,
            image_height: row.image_height,
            created_at: row.created_at,
            copy_count: row.copy_count,
            favorited: row.favorited,
            source_device: row.source_device,
        }
    }

    pub fn from_favorite(row: FavoriteSummaryRow) -> Self {
        Self {
            id: row.id,
            format: row.format,
            category: row.category,
            text: row.text,
            html: row.html,
            file_path: row.file_path,
            color: row.color,
            image_width: row.image_width,
            image_height: row.image_height,
            // 收藏列表展示的是收藏时间，而非内容的原始复制时间。
            created_at: row.favorited_at,
            copy_count: row.copy_count,
            favorited: true,
            source_device: row.source_device,
        }
    }
}

/// 为剪贴板内容生成稳定指纹，用于快速定位可合并的重复记录。
pub fn content_fingerprint(item: &NewClipboardItem) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    hash_optional_bytes(&mut hash, Some(item.format.as_bytes()));
    hash_optional_bytes(&mut hash, Some(item.category.as_bytes()));
    hash_optional_bytes(&mut hash, item.text.as_deref().map(str::as_bytes));
    hash_optional_bytes(&mut hash, item.html.as_deref().map(str::as_bytes));
    hash_optional_bytes(&mut hash, item.file_path.as_deref().map(str::as_bytes));
    hash_optional_bytes(&mut hash, item.color.as_deref().map(str::as_bytes));
    hash_optional_bytes(&mut hash, item.image.as_deref());
    hash_optional_i64(&mut hash, item.image_width);
    hash_optional_i64(&mut hash, item.image_height);
    format!("{hash:016x}")
}

fn hash_optional_i64(hash: &mut u64, value: Option<i64>) {
    match value {
        Some(number) => hash_optional_bytes(hash, Some(&number.to_le_bytes())),
        None => hash_optional_bytes(hash, None),
    }
}

fn hash_optional_bytes(hash: &mut u64, value: Option<&[u8]>) {
    match value {
        Some(bytes) => {
            hash_bytes(hash, &[1]);
            hash_bytes(hash, &(bytes.len() as u64).to_le_bytes());
            hash_bytes(hash, bytes);
        }
        None => hash_bytes(hash, &[0]),
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

/// 多设备同步：导出给对端的一条记录。
///
/// 图片默认不同步；开启后由后端压到 1280px 并编码成 PNG Base64 放进 `image`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncExportItem {
    pub id: i64,
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub color: Option<String>,
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub created_at: i64,
    pub copy_count: i64,
}

/// 同步导出查询行：比 SyncExportItem 多带原始 RGBA 字节，编码成 PNG 后即丢弃。
#[derive(Debug, Clone, FromRow)]
pub struct SyncExportRow {
    pub id: i64,
    pub format: String,
    pub category: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub color: Option<String>,
    pub file_path: Option<String>,
    pub image: Option<Vec<u8>>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub created_at: i64,
    pub copy_count: i64,
}

impl SyncExportRow {
    /// 转成对端可读的导出记录；图片按需压缩编码，体积超限的直接放弃图片。
    pub fn into_export_item(self, max_dimension: u32, max_base64_bytes: usize) -> SyncExportItem {
        let image = if self.format == "image" {
            match (self.image, self.image_width, self.image_height) {
                (Some(bytes), Some(width), Some(height)) => {
                    encode_png_bytes(bytes, width, height, Some(max_dimension))
                        .filter(|encoded| encoded.len() <= max_base64_bytes)
                }
                _ => None,
            }
        } else {
            None
        };

        SyncExportItem {
            id: self.id,
            format: self.format,
            category: self.category,
            text: self.text,
            html: self.html,
            color: self.color,
            file_path: self.file_path,
            image,
            created_at: self.created_at,
            copy_count: self.copy_count,
        }
    }
}

/// 对端推过来的一条记录，由前端解密后投递。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncImportItem {
    pub format: String,
    pub category: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    /// 图片的 PNG Base64，仅在 format = "image" 时使用
    #[serde(default)]
    pub image: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub copy_count: Option<i64>,
    /// 发送方设备名，解密后由前端带回来，落库用于在列表里标注来源
    #[serde(default)]
    pub device: Option<String>,
}

pub fn encode_png(row: ClipboardItemRow, max_dimension: Option<u32>) -> Option<String> {
    encode_png_bytes(row.image?, row.image_width?, row.image_height?, max_dimension)
}

/// RGBA 原始字节 → PNG Base64；可指定最长边，超过就先等比缩小。
pub fn encode_png_bytes(
    bytes: Vec<u8>,
    width: i64,
    height: i64,
    max_dimension: Option<u32>,
) -> Option<String> {
    let width: u32 = width.try_into().ok()?;
    let height: u32 = height.try_into().ok()?;
    let rgba = image::RgbaImage::from_raw(width, height, bytes)?;
    let mut dynamic = image::DynamicImage::ImageRgba8(rgba);
    if let Some(max_dimension) = max_dimension {
        dynamic = dynamic.thumbnail(max_dimension, max_dimension);
    }
    let mut buffer = Vec::new();
    if dynamic
        .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
        .is_err()
    {
        return None;
    }
    Some(base64::engine::general_purpose::STANDARD.encode(buffer))
}

/// 同步接收：PNG Base64 → RGBA 原始字节与尺寸。
///
/// 和 `save_png_base64` 用同一套解码限制，避免对端塞进来一张超大图把内存打爆。
pub fn decode_sync_image(encoded: &str) -> Option<(Vec<u8>, i64, i64)> {
    const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
    const MAX_ALLOC_BYTES: u64 = 64 * 1024 * 1024;

    let trimmed = encoded.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_ENCODED_BYTES {
        return None;
    }
    let png = base64::engine::general_purpose::STANDARD.decode(trimmed).ok()?;
    let mut reader = image::ImageReader::new(Cursor::new(png)).with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(MAX_ALLOC_BYTES);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), i64::from(width), i64::from(height)))
}
