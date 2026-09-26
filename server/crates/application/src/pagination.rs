//! Pagination DTOs: the query parameters a list endpoint accepts and the page it returns. The
//! strategy is in `domain::pagination`.
//!
//! A list query struct calls [`page_request`] for `limit` and `after`, the service passes the
//! resulting `PageRequest` to `CrudService::list` and converts the `Page` it gets back with
//! `PageDto::from`.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::pagination::{
    Cursor, MAX_CURSOR_BYTES, Page, PageRequest, PageSize, Sort, invalid_cursor,
};

use crate::error::ValidationErrors;

/// Validates the pagination parameters `?limit=20&after=<cursor>` for a list in `sort`. Both are
/// optional: `limit` is 1 to 100 and defaults to 20, `after` is the previous page's
/// `nextCursor`. List endpoints call this from their own query struct, because query strings
/// cannot be flattened into nested structs.
pub fn page_request<S: Sort>(
    limit: Option<u32>,
    after: Option<&str>,
    sort: S,
) -> Result<PageRequest<S>, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let size = errors.check("limit", PageSize::parse(limit));
    let after = match after.filter(|raw| !raw.is_empty()) {
        Some(raw) => errors.check("after", decode_cursor(raw, sort)).map(Some),
        None => Some(None),
    };

    match (size, after) {
        (Some(size), Some(after)) => Ok(PageRequest::sorted(size, after, sort)),
        _ => Err(errors),
    }
}

pub fn encode_cursor(cursor: &Cursor) -> String {
    URL_SAFE_NO_PAD.encode(cursor.as_bytes())
}

pub fn decode_cursor<S: Sort>(
    raw: &str,
    sort: S,
) -> Result<Cursor, domain::error::ValidationError> {
    // Four base64 characters per three bytes; anything longer cannot be a valid cursor.
    if raw.len() > MAX_CURSOR_BYTES.div_ceil(3) * 4 {
        return Err(invalid_cursor());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(raw.trim())
        .map_err(|_| invalid_cursor())?;
    let cursor = Cursor::from_bytes(bytes)?;
    if sort.accepts(&cursor) {
        Ok(cursor)
    } else {
        Err(invalid_cursor())
    }
}

/// One page of a list response, generic over the item DTO `T`. Protocol Buffers has no generics,
/// so the wire layer declares one page message per item type (such as `NotePage`) and converts
/// this.
#[derive(Debug, Clone)]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

impl<T> From<Page<T>> for PageDto<T> {
    fn from(page: Page<T>) -> Self {
        Self {
            items: page.items,
            next_cursor: page.next.as_ref().map(encode_cursor),
        }
    }
}
