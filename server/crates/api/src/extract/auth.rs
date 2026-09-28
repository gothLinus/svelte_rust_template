use std::{
    future::{Future, ready},
    marker::PhantomData,
    ops::Deref,
    sync::Arc,
};

use application::{actor::Actor, auth::Authenticated};
use axum::{extract::FromRequestParts, http::request::Parts};
use domain::{rbac::Permission, session::Session, user::User};

use crate::problem::ApiError;

#[derive(Debug, Clone)]
pub struct Authentication(pub Arc<Authenticated>);

/// The signed-in user. Rejects anonymous requests with `401`.
///
/// This proves who is calling, nothing more. Whether they may act on a particular resource is
/// decided by the application services, which take the [`Actor`].
#[derive(Debug, Clone)]
pub struct CurrentUser(Arc<Authenticated>);

impl CurrentUser {
    pub fn actor(&self) -> &Actor {
        &self.0.actor
    }

    pub fn user(&self) -> &User {
        &self.0.user
    }

    pub fn session(&self) -> &Session {
        &self.0.session
    }
}

impl<S: Send + Sync> FromRequestParts<S> for CurrentUser {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(
            parts
                .extensions
                .get::<Authentication>()
                .map(|auth| Self(Arc::clone(&auth.0)))
                .ok_or_else(ApiError::unauthenticated),
        )
    }
}

#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<CurrentUser>);

impl<S: Send + Sync> FromRequestParts<S> for OptionalUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            CurrentUser::from_request_parts(parts, state).await.ok(),
        ))
    }
}

pub trait PermissionMarker: Send + Sync + 'static {
    const PERMISSION: Permission;
}

pub mod permission {
    use domain::rbac::Permission;

    use super::PermissionMarker;

    macro_rules! markers {
        ($($variant:ident => $name:literal, $description:literal;)*) => {$(
            #[doc = $description]
            #[derive(Debug, Clone, Copy)]
            pub struct $variant;

            impl PermissionMarker for $variant {
                const PERMISSION: Permission = Permission::$variant;
            }
        )*};
    }

    domain::for_each_permission!(markers);
}

/// The signed-in user, who must hold permission `P`: `401` if nobody is signed in, `403` if the
/// user lacks it.
///
/// A route-level guard for endpoints that are all-or-nothing (the admin area). Checks that depend
/// on the resource (whose note is it?) stay in the services.
#[derive(Debug, Clone)]
pub struct RequirePermission<P: PermissionMarker> {
    user: CurrentUser,
    permission: PhantomData<P>,
}

impl<P: PermissionMarker> Deref for RequirePermission<P> {
    type Target = CurrentUser;

    fn deref(&self) -> &CurrentUser {
        &self.user
    }
}

impl<S: Send + Sync, P: PermissionMarker> FromRequestParts<S> for RequirePermission<P> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.actor().has(P::PERMISSION) {
            Ok(Self {
                user,
                permission: PhantomData,
            })
        } else {
            Err(ApiError::forbidden())
        }
    }
}
