use std::io::Cursor;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ClipboardItemRow {
    pub format: String,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<ClipboardItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageQuery {
    pub start_ts: Option<i64>,
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

pub fn encode_png(row: ClipboardItemRow, max_dimension: Option<u32>) -> Option<String> {
    let bytes = row.image?;
    let width: u32 = row.image_width?.try_into().ok()?;
    let height: u32 = row.image_height?.try_into().ok()?;
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
