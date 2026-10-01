//! `UserRepository`: accounts and their credentials columns.
//!
//! Email and username are unique regardless of case (`users_email_lower_key`,
//! `users_username_lower_key`), so lookups compare `lower(..)` and a clash surfaces as
//! `StorageError::UniqueViolation` for the service to map. `UserRow` is shared with the session
//! lookup, which joins the user in.

use domain::{
    error::{StorageError, UnknownValue},
    i18n::Locale,
    pagination::{Cursor, Page, PageRequest},
    user::{
        Email, NewUser, PasswordHash, PhoneNumber, User, UserFilter, UserId, UserParts,
        UserRepository, Username,
    },
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error, to_i64},
    postgres::{PgExecutor, PgHandle},
};

pub(super) struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub phone: Option<String>,
    pub phone_verified_at: Option<OffsetDateTime>,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<OffsetDateTime>,
    pub disabled_at: Option<OffsetDateTime>,
    pub locale: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TryFrom<UserRow> for User {
    type Error = StorageError;

    fn try_from(row: UserRow) -> Result<Self, Self::Error> {
        Ok(Self::from_parts(UserParts {
            id: UserId::from_uuid(row.id),
            email: Email::parse(&row.email).map_err(corrupt)?,
            username: Username::parse(&row.username).map_err(corrupt)?,
            phone: row
                .phone
                .as_deref()
                .map(PhoneNumber::parse)
                .transpose()
                .map_err(corrupt)?,
            phone_verified_at: row.phone_verified_at,
            password_hash: row.password_hash.map(PasswordHash::new),
            email_verified_at: row.email_verified_at,
            disabled_at: row.disabled_at,
            locale: row
                .locale
                .map(|tag| Locale::parse(&tag).ok_or_else(|| UnknownValue::new("language", tag)))
                .transpose()
                .map_err(corrupt)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }
}

impl<C: PgHandle> UserRepository for PgExecutor<C> {
    async fn create_user(&mut self, user: &NewUser) -> Result<User, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            insert into users (id, email, username, password_hash, email_verified_at, locale)
            values ($1, $2, $3, $4, $5, $6)
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            user.id.as_uuid(),
            user.email.as_str(),
            user.username.as_str(),
            user.password_hash.as_ref().map(PasswordHash::as_str),
            user.email_verified_at,
            user.locale.as_ref().map(Locale::as_str),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?
        .try_into()
    }

    async fn find_user(&mut self, id: UserId) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            select id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            from users
            where id = $1
            "#,
            id.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn find_users(&mut self, ids: &[UserId]) -> Result<Vec<User>, StorageError> {
        let ids: Vec<Uuid> = ids.iter().map(UserId::as_uuid).collect();
        sqlx::query_as!(
            UserRow,
            r#"
            select id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            from users
            where id = any($1)
            "#,
            &ids,
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?
        .into_iter()
        .map(User::try_from)
        .collect()
    }

    async fn find_user_for_update(&mut self, id: UserId) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            select id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            from users
            where id = $1
            for update
            "#,
            id.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn find_user_by_email(&mut self, email: &Email) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            select id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            from users
            where lower(email) = lower($1)
            "#,
            email.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn find_user_by_username(
        &mut self,
        username: &Username,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            select id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            from users
            where lower(username) = lower($1)
            "#,
            username.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn find_user_by_phone(
        &mut self,
        phone: &PhoneNumber,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            select id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            from users
            where phone = $1
            "#,
            phone.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn list_users(
        &mut self,
        filter: &UserFilter,
        page: PageRequest,
    ) -> Result<Page<User>, StorageError> {
        // One static query per filter shape (see the notes repository for why). A search walks the
        // table backwards from the cursor: fine for an admin page with tens of thousands of users
        // (97 ms on 200k when nothing matches). Beyond that, add trigram indexes in a migration
        // (`pg_trgm`, gin on `lower(email)` and `username`) and search with `like`. `strpos` is
        // used instead of `like` so `%` and `_` in the term are literal.
        let after = page.after_id().unwrap_or(Uuid::max());
        let limit = to_i64(page.fetch_limit());
        let rows = match filter.search.as_deref() {
            Some(search) => {
                sqlx::query_as!(
                    UserRow,
                    r#"
                    select id, email, username, phone, phone_verified_at, password_hash,
                        email_verified_at, disabled_at, locale, created_at, updated_at
                    from users
                    where id < $1
                      and (strpos(lower(email), lower($2)) > 0
                           or strpos(username, lower($2)) > 0)
                    order by id desc
                    limit $3
                    "#,
                    after,
                    search,
                    limit,
                )
                .fetch_all(self.conn())
                .await
            }
            None => {
                sqlx::query_as!(
                    UserRow,
                    r#"
                    select id, email, username, phone, phone_verified_at, password_hash,
                        email_verified_at, disabled_at, locale, created_at, updated_at
                    from users
                    where id < $1
                    order by id desc
                    limit $2
                    "#,
                    after,
                    limit,
                )
                .fetch_all(self.conn())
                .await
            }
        }
        .map_err(db_error)?
        .into_iter()
        .map(User::try_from)
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::from_rows(rows, &page, |user| {
            Cursor::from_uuid(user.id().as_uuid())
        }))
    }

    async fn set_user_username(
        &mut self,
        id: UserId,
        username: &Username,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            update users set username = $2
            where id = $1
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            id.as_uuid(),
            username.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn set_user_locale(
        &mut self,
        id: UserId,
        locale: Option<&Locale>,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            update users set locale = $2
            where id = $1
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            id.as_uuid(),
            locale.map(Locale::as_str),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn change_user_email(
        &mut self,
        id: UserId,
        email: &Email,
        verified_at: OffsetDateTime,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            update users set email = $2, email_verified_at = $3
            where id = $1
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            id.as_uuid(),
            email.as_str(),
            verified_at,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn set_user_phone(
        &mut self,
        id: UserId,
        phone: Option<(&PhoneNumber, OffsetDateTime)>,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            update users set phone = $2, phone_verified_at = $3
            where id = $1
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            id.as_uuid(),
            phone.map(|(phone, _)| phone.as_str()),
            phone.map(|(_, at)| at),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn set_user_password(
        &mut self,
        id: UserId,
        hash: Option<&PasswordHash>,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "update users set password_hash = $2 where id = $1",
            id.as_uuid(),
            hash.map(PasswordHash::as_str),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn mark_user_email_verified(
        &mut self,
        id: UserId,
        at: OffsetDateTime,
    ) -> Result<Option<User>, StorageError> {
        sqlx::query_as!(
            UserRow,
            r#"
            update users set email_verified_at = coalesce(email_verified_at, $2)
            where id = $1
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            id.as_uuid(),
            at,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn set_user_disabled(
        &mut self,
        id: UserId,
        at: Option<OffsetDateTime>,
    ) -> Result<Option<User>, StorageError> {
        // Disabling an already disabled account keeps the original timestamp.
        sqlx::query_as!(
            UserRow,
            r#"
            update users
            set disabled_at = case when $2::timestamptz is null then null
                                   else coalesce(disabled_at, $2) end
            where id = $1
            returning id, email, username, phone, phone_verified_at, password_hash,
                email_verified_at, disabled_at, locale, created_at, updated_at
            "#,
            id.as_uuid(),
            at,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(User::try_from)
        .transpose()
    }

    async fn delete_user(&mut self, id: UserId) -> Result<bool, StorageError> {
        let result = sqlx::query!("delete from users where id = $1", id.as_uuid())
            .execute(self.conn())
            .await
            .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_unverified_users(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from users
                where id in (
                    select u.id from users u
                    where u.email_verified_at is null and u.created_at < $1
                      and not exists (
                          select 1 from user_roles r where r.user_id = u.id and r.role <> 'user'
                      )
                    limit $2
                )
                "#,
                cutoff,
                CLEANUP_BATCH,
            )
            .execute(self.conn())
            .await
            .map_err(db_error)?
            .rows_affected();
            total += deleted;
            if cleanup_done(deleted) {
                return Ok(total);
            }
        }
    }
}
