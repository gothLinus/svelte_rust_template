//! Authorization policies: who may do what to which resource.
//!
//! Handlers only check that a user is signed in (and, for whole route groups, that they hold a
//! permission). Whether the user may act on *this* resource is decided here, where the loaded
//! resource is available. The UI's permission checks only hide buttons; these are the ones that
//! count.
//!
//! A resource gets a policy by implementing [`SimplePolicy`] (permissions and the resource
//! decide) or, when the rules need more data, [`Policy`] itself. `CrudService` runs it on every
//! operation; a hand-written use case reaches it through `CrudService::authorize`.

use std::future::{Future, ready};

use domain::{error::StorageError, rbac::Permission, repository::Resource, user::UserId};

use crate::{Store, actor::Actor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Read,
    Create,
    Update,
    Delete,
}

pub trait Owned {
    fn owner_id(&self) -> UserId;
}

/// Decides what an actor may do with resources of type `E`.
///
/// Policies are zero-sized types with associated functions, so being a type parameter of
/// `CrudService` means the checks are resolved at compile time.
///
/// Rules that need nothing but the actor's permissions and the resource implement
/// [`SimplePolicy`] instead, which provides this trait (a type cannot implement both). Rules
/// about relationships (project members, shared documents, tenants) implement this trait
/// directly: [`Policy::facts`] loads what they need, once per operation and through the same
/// store and transaction, and the two synchronous hooks then decide from it.
///
/// `CrudService` calls `facts`, then `allows` (for a single resource first with
/// [`Action::Read`], failing as `NotFound`, then with the requested action, failing as
/// `Forbidden`), and `narrow` for lists.
pub trait Policy<E: Resource>: Send + Sync + 'static {
    type Facts: Send + Sync;

    fn facts(
        store: &mut impl Store,
        actor: &Actor,
        resource: Option<&E>,
    ) -> impl Future<Output = Result<Self::Facts, StorageError>> + Send;

    fn allows(actor: &Actor, facts: &Self::Facts, action: Action, resource: Option<&E>) -> bool;

    /// Checks a list filter against what the actor may see: returns it as is, or narrowed, or
    /// `None` if the actor asked for more than they may see (the list is then `Forbidden`). A
    /// policy that should quietly show only the actor's own rows returns the filter with the owner
    /// set instead.
    fn narrow(actor: &Actor, facts: &Self::Facts, filter: E::Filter) -> Option<E::Filter>;
}

/// A policy that decides from the actor's permissions and the resource alone, like the notes'
/// "owner or manager" rule. Every `SimplePolicy` is a [`Policy`] without facts:
/// [`can`](SimplePolicy::can) is its `allows` and [`scope`](SimplePolicy::scope) its `narrow`.
pub trait SimplePolicy<E: Resource>: Send + Sync + 'static {
    fn can(actor: &Actor, action: Action, resource: Option<&E>) -> bool;

    fn scope(actor: &Actor, filter: E::Filter) -> Option<E::Filter>;
}

impl<E: Resource, P: SimplePolicy<E>> Policy<E> for P {
    type Facts = ();

    fn facts(
        _store: &mut impl Store,
        _actor: &Actor,
        _resource: Option<&E>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(Ok(()))
    }

    fn allows(actor: &Actor, (): &(), action: Action, resource: Option<&E>) -> bool {
        P::can(actor, action, resource)
    }

    fn narrow(actor: &Actor, (): &(), filter: E::Filter) -> Option<E::Filter> {
        P::scope(actor, filter)
    }
}

/// The usual rule for owned resources: the `any` permission covers everyone's, the `own`
/// permission covers the actor's own. For use inside [`SimplePolicy::can`].
pub fn owner_or(actor: &Actor, resource: &impl Owned, own: Permission, any: Permission) -> bool {
    actor.has(any) || (actor.has(own) && resource.owner_id() == actor.user_id)
}
