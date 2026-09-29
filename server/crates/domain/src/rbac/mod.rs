//! Role-based access control: roles bundle [`Permission`]s, users hold roles.
//!
//! The data model lives in the database so roles can change without a deploy. Permissions are
//! code: each is a variant of [`Permission`], because the code that checks it has to exist
//! anyway. Policies in `application` check permissions, never role names, against the
//! [`PermissionSet`] loaded with the session; [`RbacRepository`] stores roles and who holds them.
//!
//! To add a permission, add one line to [`for_each_permission!`](crate::for_each_permission) and
//! a migration that inserts it and grants it to `admin` (and to `user`, together with
//! [`DEFAULT_USER_PERMISSIONS`], if everyone gets it). The protobuf `Permission` enum in
//! `proto/api/v1/common.proto` and the wire conversion in `wire/common.rs` list them too;
//! `just new-resource` updates all of these for a new resource.

pub use permission::{Permission, PermissionSet};
pub use repository::RbacRepository;
pub use role::{
    DEFAULT_USER_PERMISSIONS, MAX_ROLE_NAME_LEN, Role, RoleName, default_user_permissions,
};

mod permission;
mod repository;
mod role;
