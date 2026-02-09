pub struct TextClassification {
  pub format: String,
  pub category: String,
  pub color: Option<String>,
  pub file_path: Option<String>,
}

pub fn classify_text(input: &str) -> TextClassification {
  let trimmed = input.trim();
  if let Some(color) = detect_color(trimmed) {
    return TextClassification {
      format: "color".to_string(),
      category: "text".to_string(),
      color: Some(color),
      file_path: None,
    };
  }

  if looks_like_file_path(trimmed) {
    return TextClassification {
      format: "file".to_string(),
      category: "file".to_string(),
      color: None,
      file_path: Some(trimmed.to_string()),
    };
  }

  let category = if looks_like_url(trimmed) {
    "link"
  } else {
    "text"
  };

  TextClassification {
    format: "text".to_string(),
    category: category.to_string(),
    color: None,
    file_path: None,
  }
}

pub fn looks_like_url(text: &str) -> bool {
  let lower = text.to_ascii_lowercase();
  lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn looks_like_file_path(text: &str) -> bool {
  let trimmed = text.trim();
  if trimmed.starts_with("\\\\") {
    return true;
  }
  if trimmed.len() >= 3 {
    let bytes = trimmed.as_bytes();
    if bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
      return true;
    }
  }
  trimmed.starts_with('/')
}

pub fn detect_color(text: &str) -> Option<String> {
  let trimmed = text.trim();
  let hex = trimmed.strip_prefix('#')?;
  let len = hex.len();
  if len != 3 && len != 4 && len != 6 && len != 8 {
    return None;
  }
  if hex.chars().all(|c| c.is_ascii_hexdigit()) {
    return Some(trimmed.to_string());
  }
  None
}

#[allow(dead_code)]
pub fn strip_html(input: &str) -> String {
  let mut output = String::with_capacity(input.len());
  let mut in_tag = false;
  for ch in input.chars() {
    match ch {
      '<' => in_tag = true,
      '>' => in_tag = false,
      _ if !in_tag => output.push(ch),
      _ => {}
    }
  }
  output
}
