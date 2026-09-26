use std::{
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    marker::PhantomData,
    str::FromStr,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
};

use time::OffsetDateTime;
use uuid::{Builder, Uuid};

use crate::{error::ValidationError, i18n::Message};

/// A UUID tagged with the entity it identifies, so a `UserId` cannot be passed where a
/// `NoteId` is expected.
///
/// New ids are `UUIDv7`, so they sort by creation time; keyset pagination relies on that. `T` is only a marker and puts no bounds on the
/// traits `Id` implements.
///
/// ```
/// use domain::{id::Id, user::User};
///
/// let id = Id::<User>::generate();
/// assert_eq!(id.to_string().parse::<Id<User>>().unwrap(), id);
/// ```
pub struct Id<T> {
    uuid: Uuid,
    entity: PhantomData<fn() -> T>,
}

static SEQUENCE: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(random_tail() & 0x3ff_ffff));

impl<T> Id<T> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "each byte of the tail takes the bits of the sequence shifted into it"
    )]
    pub fn generate_at(at: OffsetDateTime) -> Self {
        let millis = u64::try_from(at.unix_timestamp_nanos().max(0) / 1_000_000).unwrap_or(0);
        let sequence = SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
        // The 74 bits after the version: the sequence in the 42 that the version and variant leave
        // whole, then 32 random bits.
        let random = random_tail().to_be_bytes();
        let mut tail = [0u8; 10];
        tail[0] = ((sequence >> 38) & 0x0f) as u8;
        tail[1] = (sequence >> 30) as u8;
        tail[2] = ((sequence >> 24) & 0x3f) as u8;
        tail[3] = (sequence >> 16) as u8;
        tail[4] = (sequence >> 8) as u8;
        tail[5] = sequence as u8;
        tail[6..].copy_from_slice(&random[4..]);
        Self::from_uuid(Builder::from_unix_timestamp_millis(millis, &tail).into_uuid())
    }

    pub fn generate() -> Self {
        Self::from_uuid(Uuid::now_v7())
    }

    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self {
            uuid,
            entity: PhantomData,
        }
    }

    pub const fn as_uuid(&self) -> Uuid {
        self.uuid
    }

    /// Parses any UUID text form, ignoring surrounding whitespace.
    ///
    /// Accepts a UUID of any version, so it says nothing about whether the entity exists.
    ///
    /// # Errors
    ///
    /// Fails with `invalid_id` if `raw` is not a UUID.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        Uuid::try_parse(raw.trim())
            .map(Self::from_uuid)
            .map_err(|_| ValidationError::new("invalid_id", Message::new("validation-invalid-id")))
    }
}

// Implemented by hand: deriving would require `T` itself to implement each trait.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl<T> Debug for Id<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.uuid.hyphenated())
    }
}

impl<T> Display for Id<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.uuid.hyphenated(), f)
    }
}

impl<T> FromStr for Id<T> {
    type Err = ValidationError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

impl<T> From<Id<T>> for Uuid {
    fn from(id: Id<T>) -> Self {
        id.uuid
    }
}

fn random_tail() -> u64 {
    let bytes = Uuid::now_v7().into_bytes();
    u64::from_be_bytes([0, 0, 0, 0, bytes[12], bytes[13], bytes[14], bytes[15]])
}

/// An id type that new entities get from their creation time: [`Id`] as a bound for generic code
/// such as [`Resource::Id`](crate::repository::Resource::Id), where the entity is not known.
pub trait NewId: Sized {
    fn generate_at(at: OffsetDateTime) -> Self;
}

impl<T> NewId for Id<T> {
    fn generate_at(at: OffsetDateTime) -> Self {
        Id::generate_at(at)
    }
}
