use std::collections::HashSet;

use domain::rbac::{Permission, PermissionSet, RoleName};

#[test]
fn every_permission_is_listed_once_in_all() {
    // `ALL` is generated from the same list as the enum, so it cannot miss a variant; this guards
    // against a variant listed twice.
    let unique: HashSet<_> = Permission::ALL.iter().collect();
    assert_eq!(unique.len(), Permission::ALL.len());
}

#[test]
fn permission_names_round_trip() {
    for permission in Permission::ALL {
        assert_eq!(Permission::parse(permission.as_str()).unwrap(), permission);
        assert_eq!(permission.to_string(), permission.as_str());
        assert_eq!(
            permission.as_str().parse::<Permission>().unwrap(),
            permission
        );
        assert!(!permission.description().is_empty());
    }
}

#[test]
fn permission_names_follow_resource_action() {
    for permission in Permission::ALL {
        let (resource, action) = permission.as_str().split_once(':').unwrap();
        assert!(!resource.is_empty() && !action.is_empty());
    }
}

#[test]
fn unknown_permission_is_rejected() {
    assert_eq!(
        Permission::parse("notes:delete").unwrap_err().code(),
        "unknown_permission"
    );
}

#[test]
fn permission_set_operations() {
    let mut set = PermissionSet::empty();
    assert!(set.is_empty());

    set.insert(Permission::NotesWrite);
    set.insert(Permission::NotesRead);
    set.insert(Permission::NotesRead);

    assert!(set.contains(Permission::NotesRead));
    assert!(!set.contains(Permission::UsersManage));
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        [Permission::NotesRead, Permission::NotesWrite]
    );
    assert_eq!(format!("{set:?}"), "{notes:read, notes:write}");
}

#[test]
fn permission_set_all_contains_everything() {
    let all = PermissionSet::all();
    assert!(Permission::ALL.iter().all(|p| all.contains(*p)));
    assert_eq!(all.iter().count(), Permission::ALL.len());
    assert_eq!(Permission::ALL.into_iter().collect::<PermissionSet>(), all);
}

#[test]
fn role_names() {
    assert_eq!(RoleName::ADMIN.as_str(), "admin");
    assert_eq!(RoleName::USER.to_string(), "user");
    assert_eq!(RoleName::parse("support-2").unwrap().as_str(), "support-2");
    assert_eq!(RoleName::parse("admin").unwrap(), RoleName::ADMIN);

    for raw in ["", "Admin", "2fa", "a b", "a:b", &"a".repeat(51)] {
        assert_eq!(
            RoleName::parse(raw).unwrap_err().code(),
            "invalid_role",
            "{raw}"
        );
    }
}
