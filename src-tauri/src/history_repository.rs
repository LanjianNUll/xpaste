use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::models::ClipboardItemSummaryRow;

pub struct HistoryFilter<'a> {
    pub start_ts: Option<i64>,
    pub end_ts: Option<i64>,
    pub keyword: &'a str,
    pub formats: &'a [String],
    pub categories: &'a [String],
}

/// 使用数据库分页和过滤，避免将完整历史记录加载到 UI 内存。
pub async fn query_page(
    pool: &SqlitePool,
    filter: &HistoryFilter<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ClipboardItemSummaryRow>, sqlx::Error> {
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, format, category, substr(text, 1, 4096) AS text, \
         substr(html, 1, 4096) AS html, file_path, color, \
         image_width, image_height, created_at, copy_count FROM clipboard_items WHERE 1 = 1",
    );
    push_filters(&mut builder, filter);
    builder.push(" ORDER BY created_at DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);
    builder
        .build_query_as::<ClipboardItemSummaryRow>()
        .fetch_all(pool)
        .await
}

pub async fn count(pool: &SqlitePool, filter: &HistoryFilter<'_>) -> Result<i64, sqlx::Error> {
    let mut builder =
        QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM clipboard_items WHERE 1 = 1");
    push_filters(&mut builder, filter);
    builder.build_query_scalar::<i64>().fetch_one(pool).await
}

fn push_filters<'a>(builder: &mut QueryBuilder<'a, Sqlite>, filter: &'a HistoryFilter<'a>) {
    if let Some(start_ts) = filter.start_ts {
        builder.push(" AND created_at >= ").push_bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        builder.push(" AND created_at <= ").push_bind(end_ts);
    }
    let keyword = filter.keyword.trim();
    if !keyword.is_empty() {
        let pattern = format!("%{keyword}%");
        builder
            .push(" AND (text LIKE ")
            .push_bind(pattern.clone())
            .push(" OR html LIKE ")
            .push_bind(pattern.clone())
            .push(" OR file_path LIKE ")
            .push_bind(pattern.clone())
            .push(" OR color LIKE ")
            .push_bind(pattern)
            .push(")");
    }
    push_format_filter(builder, filter.formats);
    push_in_filter(builder, "category", filter.categories);
}

fn push_format_filter<'a>(builder: &mut QueryBuilder<'a, Sqlite>, formats: &'a [String]) {
    if formats.is_empty() {
        return;
    }
    let has_link = formats.iter().any(|format| format == "link");
    let regular = formats
        .iter()
        .filter(|format| format.as_str() != "link")
        .collect::<Vec<_>>();
    builder.push(" AND (");
    if !regular.is_empty() {
        builder.push("format IN (");
        let mut values = builder.separated(", ");
        for format in regular {
            values.push_bind(format);
        }
        values.push_unseparated(")");
        if has_link {
            builder.push(" OR ");
        }
    }
    if has_link {
        builder.push("category = 'link'");
    }
    builder.push(")");
}

fn push_in_filter<'a>(
    builder: &mut QueryBuilder<'a, Sqlite>,
    column: &'a str,
    values: &'a [String],
) {
    if values.is_empty() {
        return;
    }
    builder.push(" AND ").push(column).push(" IN (");
    let mut separated = builder.separated(", ");
    for value in values {
        separated.push_bind(value);
    }
    separated.push_unseparated(")");
}
