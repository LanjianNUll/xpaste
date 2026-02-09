use std::io::Cursor;

use base64::Engine;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ClipboardItemRow {
  pub id: i64,
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
  pub image_base64: Option<String>,
  pub image_width: Option<i64>,
  pub image_height: Option<i64>,
  pub created_at: i64,
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
  pub fn from_row(row: ClipboardItemRow) -> Self {
    let image_base64 = encode_png(&row.image, row.image_width, row.image_height);
    Self {
      id: row.id,
      format: row.format,
      category: row.category,
      text: row.text,
      html: row.html,
      file_path: row.file_path,
      color: row.color,
      image_base64,
      image_width: row.image_width,
      image_height: row.image_height,
      created_at: row.created_at,
    }
  }
}

fn encode_png(image: &Option<Vec<u8>>, width: Option<i64>, height: Option<i64>) -> Option<String> {
  let bytes = image.as_ref()?;
  let width: u32 = width?.try_into().ok()?;
  let height: u32 = height?.try_into().ok()?;
  let rgba = image::RgbaImage::from_raw(width, height, bytes.clone())?;
  let dynamic = image::DynamicImage::ImageRgba8(rgba);
  let mut buffer = Vec::new();
  if dynamic
    .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
    .is_err()
  {
    return None;
  }
  Some(base64::engine::general_purpose::STANDARD.encode(buffer))
}
