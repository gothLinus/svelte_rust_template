//! Keyset pagination.
//!
//! A list is ordered by a [`Sort`], by default [`NewestFirst`]. Ids are `UUIDv7`, so id order is
//! creation order, and "the next page" is "the rows after the last one I saw". That query uses
//! an index, costs the same on every page, and never skips or repeats a row when others are
//! inserted in between.
//!
//! Where the next page starts is a [`Cursor`]: the position of the last row, as opaque bytes
//! that only the server reads.

use std::fmt::Debug;

use uuid::Uuid;

use crate::{error::ValidationError, i18n::Message};

pub const DEFAULT_PAGE_SIZE: u32 = 20;
pub const MAX_PAGE_SIZE: u32 = 100;
pub const MAX_CURSOR_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSize(u32);

impl PageSize {
    /// `None` means [`DEFAULT_PAGE_SIZE`].
    ///
    /// # Errors
    ///
    /// Fails with `out_of_range` for 0 and for more than [`MAX_PAGE_SIZE`].
    pub fn parse(size: Option<u32>) -> Result<Self, ValidationError> {
        let size = size.unwrap_or(DEFAULT_PAGE_SIZE);
        if (1..=MAX_PAGE_SIZE).contains(&size) {
            Ok(Self(size))
        } else {
            Err(ValidationError::new(
                "out_of_range",
                Message::new("validation-page-size-out-of-range")
                    .arg("min", 1)
                    .arg("max", MAX_PAGE_SIZE),
            ))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for PageSize {
    fn default() -> Self {
        Self(DEFAULT_PAGE_SIZE)
    }
}

/// Where the next page starts: the position of the last item on the previous one, in the list's
/// [`Sort`]. Opaque to clients; the API encodes it as base64url.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cursor(Box<[u8]>);

const ID_BYTES: usize = 16;

impl Cursor {
    /// A cursor from its raw bytes, such as decoded from a request.
    ///
    /// # Errors
    ///
    /// Fails with `invalid_cursor` if the bytes are empty or longer than [`MAX_CURSOR_BYTES`].
    pub fn from_bytes(bytes: impl Into<Box<[u8]>>) -> Result<Self, ValidationError> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() > MAX_CURSOR_BYTES {
            return Err(invalid_cursor());
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(Box::new(*id.as_bytes()))
    }

    pub fn to_uuid(&self) -> Option<Uuid> {
        Uuid::from_slice(&self.0).ok()
    }

    /// The position of a row in an order by `key`, then id. Encode `key` so that its bytes compare
    /// like the value: big-endian numbers, or `time`'s unix nanoseconds.
    pub fn keyset(key: &[u8], id: Uuid) -> Self {
        Self([key, id.as_bytes().as_slice()].concat().into_boxed_slice())
    }

    pub fn split_keyset(&self) -> Option<(&[u8], Uuid)> {
        let split = self.0.len().checked_sub(ID_BYTES)?;
        let (key, id) = self.0.split_at(split);
        Some((key, Uuid::from_slice(id).ok()?))
    }
}

pub fn invalid_cursor() -> ValidationError {
    ValidationError::new("invalid_cursor", Message::new("validation-invalid-cursor"))
}

/// An order a resource's lists can come in, named by
/// [`Resource::Sort`](crate::repository::Resource::Sort). Each order has its own cursor format.
///
/// Usually an enum with one variant per order; `Default` is the order used when the client
/// picks none. An implementor guarantees that:
///
/// - the query orders by a strict total order (the sort column, then the id as tie-breaker,
///   see [`Cursor::keyset`]), so no row is skipped or repeated between pages;
/// - [`Sort::accepts`] admits exactly the cursors the repository can decode for that order,
///   since the repository trusts an accepted cursor;
/// - the order has an index on `(column, id)`, or lists degrade to a scan.
///
/// ```
/// use domain::pagination::{Cursor, Sort};
///
/// /// Newest first by id, or most recently updated first. The cursor of the second is an
/// /// 8-byte timestamp followed by the id.
/// #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
/// enum TagOrder {
///     #[default]
///     Newest,
///     Updated,
/// }
///
/// impl Sort for TagOrder {
///     fn accepts(self, cursor: &Cursor) -> bool {
///         match self {
///             Self::Newest => cursor.to_uuid().is_some(),
///             Self::Updated => matches!(cursor.split_keyset(), Some((key, _)) if key.len() == 8),
///         }
///     }
/// }
/// ```
pub trait Sort: Default + Copy + Debug + PartialEq + Send + Sync + 'static {
    /// Whether `cursor` can be a position in this order. Checked when a request comes in, so a
    /// cursor from another order, or a made-up one, is a `400` and never reaches a query.
    fn accepts(self, cursor: &Cursor) -> bool;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NewestFirst;

impl Sort for NewestFirst {
    fn accepts(self, cursor: &Cursor) -> bool {
        cursor.to_uuid().is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageRequest<S = NewestFirst> {
    pub size: PageSize,
    pub after: Option<Cursor>,
    pub sort: S,
}

impl<S: Sort> PageRequest<S> {
    pub fn new(size: PageSize, after: Option<Cursor>) -> Self {
        Self::sorted(size, after, S::default())
    }

    pub fn sorted(size: PageSize, after: Option<Cursor>, sort: S) -> Self {
        Self { size, after, sort }
    }
}

impl<S> PageRequest<S> {
    /// How many rows a repository should fetch: one more than the page holds, so the extra row
    /// shows whether another page exists without a `COUNT(*)`.
    pub const fn fetch_limit(&self) -> u32 {
        self.size.get() + 1
    }

    pub fn after_id(&self) -> Option<Uuid> {
        self.after.as_ref().and_then(Cursor::to_uuid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<Cursor>,
}

impl<T> Page<T> {
    /// Builds a page from up to [`PageRequest::fetch_limit`] rows in page order; the extra row only
    /// signals that there is more and is dropped. `cursor` is called on the last row kept, and only
    /// if there is a next page.
    pub fn from_rows<S>(
        mut rows: Vec<T>,
        request: &PageRequest<S>,
        cursor: impl Fn(&T) -> Cursor,
    ) -> Self {
        let size = request.size.get() as usize;
        let has_more = rows.len() > size;
        rows.truncate(size);
        let next = if has_more {
            rows.last().map(cursor)
        } else {
            None
        };
        Self { items: rows, next }
    }

    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next: None,
        }
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            next: self.next,
        }
    }
}
