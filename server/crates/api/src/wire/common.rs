use application::dto::{
    MeDto, MfaChallengeDto, MfaMethod, PermissionName, RoleDto, SessionDto, UserDto,
};
use proto::v1;

use super::{IntoMessage, timestamp};

/// The `common.proto` enum value for a domain permission. A resource that brings permissions adds
/// an arm here and a value in `common.proto`.
pub fn permission(permission: PermissionName) -> v1::Permission {
    // Exhaustive: a permission added to the domain needs a value in common.proto.
    match permission {
        PermissionName::NotesRead => v1::Permission::NotesRead,
        PermissionName::NotesWrite => v1::Permission::NotesWrite,
        PermissionName::NotesManage => v1::Permission::NotesManage,
        PermissionName::FilesRead => v1::Permission::FilesRead,
        PermissionName::FilesWrite => v1::Permission::FilesWrite,
        PermissionName::FilesManage => v1::Permission::FilesManage,
        PermissionName::UsersRead => v1::Permission::UsersRead,
        PermissionName::UsersManage => v1::Permission::UsersManage,
        PermissionName::AuditRead => v1::Permission::AuditRead,
    }
}

fn permissions(permissions: Vec<PermissionName>) -> Vec<i32> {
    permissions
        .into_iter()
        .map(|name| permission(name).into())
        .collect()
}

impl IntoMessage for UserDto {
    type Message = v1::User;

    fn into_message(self) -> v1::User {
        v1::User {
            id: self.id.to_string(),
            email: self.email,
            username: self.username,
            phone: self.phone,
            phone_verified: self.phone_verified,
            email_verified: self.email_verified,
            has_password: self.has_password,
            disabled: self.disabled,
            roles: self.roles,
            created_at: Some(timestamp(self.created_at)),
            locale: self.locale,
        }
    }
}

impl IntoMessage for MeDto {
    type Message = v1::Me;

    fn into_message(self) -> v1::Me {
        v1::Me {
            user: Some(self.user.into_message()),
            permissions: permissions(self.permissions),
        }
    }
}

impl IntoMessage for RoleDto {
    type Message = v1::Role;

    fn into_message(self) -> v1::Role {
        v1::Role {
            name: self.name,
            description: self.description,
            permissions: permissions(self.permissions),
        }
    }
}

impl IntoMessage for Vec<RoleDto> {
    type Message = v1::RoleList;

    fn into_message(self) -> v1::RoleList {
        v1::RoleList {
            roles: self.into_iter().map(IntoMessage::into_message).collect(),
        }
    }
}

impl IntoMessage for SessionDto {
    type Message = v1::Session;

    fn into_message(self) -> v1::Session {
        v1::Session {
            id: self.id.to_string(),
            current: self.current,
            ip: self.ip,
            user_agent: self.user_agent,
            created_at: Some(timestamp(self.created_at)),
            last_seen_at: Some(timestamp(self.last_seen_at)),
            expires_at: Some(timestamp(self.expires_at)),
        }
    }
}

impl IntoMessage for Vec<SessionDto> {
    type Message = v1::SessionList;

    fn into_message(self) -> v1::SessionList {
        v1::SessionList {
            sessions: self.into_iter().map(IntoMessage::into_message).collect(),
        }
    }
}

fn mfa_method(method: MfaMethod) -> v1::MfaMethod {
    match method {
        MfaMethod::Totp => v1::MfaMethod::Totp,
        MfaMethod::Passkey => v1::MfaMethod::Passkey,
        MfaMethod::RecoveryCode => v1::MfaMethod::RecoveryCode,
    }
}

impl IntoMessage for MfaChallengeDto {
    type Message = v1::MfaChallenge;

    fn into_message(self) -> v1::MfaChallenge {
        v1::MfaChallenge {
            methods: self
                .methods
                .into_iter()
                .map(|method| mfa_method(method).into())
                .collect(),
        }
    }
}
