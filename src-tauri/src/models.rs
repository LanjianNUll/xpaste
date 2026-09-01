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
        }
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
