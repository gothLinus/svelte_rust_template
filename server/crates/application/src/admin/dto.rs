use domain::{
    i18n::Message,
    pagination::{NewestFirst, PageRequest},
    user::UserFilter,
};
use serde::Deserialize;

use crate::{error::ValidationErrors, pagination::page_request};

pub const MAX_SEARCH_LEN: usize = 100;
/// Shorter terms match most of the table and cannot use a trigram index, should one be added (see
/// the users repository).
pub const MIN_SEARCH_LEN: usize = 3;

/// Query string of the user list: `?search=ali&limit=20&after=<cursor>`. Converts into the filter
/// and page request, validating both.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUsersQuery {
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub after: Option<String>,
}

impl TryFrom<ListUsersQuery> for (UserFilter, PageRequest) {
    type Error = ValidationErrors;

    fn try_from(query: ListUsersQuery) -> Result<Self, Self::Error> {
        let page = page_request(query.limit, query.after.as_deref(), NewestFirst);
        let search = query
            .search
            .map(|search| search.trim().to_owned())
            .filter(|search| !search.is_empty());

        let mut errors = page.as_ref().err().cloned().unwrap_or_default();
        let length = search.as_ref().map(|search| search.chars().count());
        if length.is_some_and(|length| length > MAX_SEARCH_LEN) {
            errors.add(
                "search",
                &domain::error::ValidationError::new(
                    "too_long",
                    Message::new("validation-search-too-long").arg("max", MAX_SEARCH_LEN),
                ),
            );
        }
        if length.is_some_and(|length| length < MIN_SEARCH_LEN) {
            errors.add(
                "search",
                &domain::error::ValidationError::new(
                    "too_short",
                    Message::new("validation-search-too-short").arg("min", MIN_SEARCH_LEN),
                ),
            );
        }

        match page {
            Ok(page) if errors.is_empty() => Ok((UserFilter { search }, page)),
            _ => Err(errors),
        }
    }
}
