use application::admin::dto::ListUsersQuery;
use domain::{session::ClientInfo, user::UserId};

use crate::support::Fixture;

#[tokio::test]
async fn listing_users_needs_users_read() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let err = fx
        .services
        .admin
        .list_users(&alice.actor, ListUsersQuery::default())
        .await
        .unwrap_err();

    assert_eq!(err.code(), "forbidden");
}

#[tokio::test]
async fn admins_list_users_newest_first_with_roles_and_pages() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    for name in ["alice", "bob", "carol"] {
        fx.register(&format!("{name}@example.com")).await;
    }

    let first = fx
        .services
        .admin
        .list_users(
            &admin.actor,
            ListUsersQuery {
                limit: Some(2),
                ..ListUsersQuery::default()
            },
        )
        .await
        .unwrap();
    let emails: Vec<&str> = first.items.iter().map(|u| u.email.as_str()).collect();
    assert_eq!(emails, ["carol@example.com", "bob@example.com"]);
    assert_eq!(first.items[0].roles, ["user"]);

    let second = fx
        .services
        .admin
        .list_users(
            &admin.actor,
            ListUsersQuery {
                limit: Some(2),
                after: first.next_cursor,
                ..ListUsersQuery::default()
            },
        )
        .await
        .unwrap();
    let emails: Vec<&str> = second.items.iter().map(|u| u.email.as_str()).collect();
    assert_eq!(emails, ["alice@example.com", "admin@example.com"]);
    assert_eq!(second.items[1].roles, ["admin", "user"]);
    assert!(second.next_cursor.is_none());
}

#[tokio::test]
async fn user_search_and_its_validation() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    fx.register("alice@example.com").await;

    let page = fx
        .services
        .admin
        .list_users(
            &admin.actor,
            ListUsersQuery {
                search: Some(" ALI ".to_owned()),
                ..ListUsersQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);

    let err = fx
        .services
        .admin
        .list_users(
            &admin.actor,
            ListUsersQuery {
                search: Some("x".repeat(101)),
                limit: Some(0),
                after: Some("garbage".to_owned()),
            },
        )
        .await
        .unwrap_err();
    let application::AppError::Validation(errors) = err else {
        panic!("expected a validation error")
    };
    let fields: Vec<&str> = errors.fields().iter().map(|e| e.field.as_str()).collect();
    assert_eq!(fields, ["limit", "after", "search"]);
}

#[tokio::test]
async fn get_user_and_list_roles() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;

    let user = fx
        .services
        .admin
        .get_user(&admin.actor, alice.actor.user_id)
        .await
        .unwrap();
    assert_eq!(user.email, "alice@example.com");

    let err = fx
        .services
        .admin
        .get_user(&admin.actor, UserId::generate())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");

    let roles = fx
        .services
        .admin
        .list_roles(&admin.actor, &domain::i18n::Locale::EN)
        .await
        .unwrap();
    let names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["admin", "user"]);
}

#[tokio::test]
async fn granting_a_role_takes_effect_and_flags_sessions_for_rotation() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;

    let user = fx
        .services
        .admin
        .grant_role(&admin.actor, alice.actor.user_id, "admin")
        .await
        .unwrap();

    assert_eq!(user.roles, ["admin", "user"]);
    assert!(
        fx.db
            .with(|state| state.sessions[&alice.session.id()].rotation_pending())
    );
}

#[tokio::test]
async fn granting_needs_users_manage_and_a_known_role_and_user() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;

    let err = fx
        .services
        .admin
        .grant_role(&alice.actor, alice.actor.user_id, "admin")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");

    for role in ["superuser", "Not A Role"] {
        let err = fx
            .services
            .admin
            .grant_role(&admin.actor, alice.actor.user_id, role)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "not_found", "{role}");
    }

    let err = fx
        .services
        .admin
        .grant_role(&admin.actor, UserId::generate(), "admin")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn the_last_admin_role_cannot_be_revoked() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let err = fx
        .services
        .admin
        .revoke_role(&admin.actor, admin.actor.user_id, "admin")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "last_admin");

    let other = fx.admin("other@example.com").await;
    let user = fx
        .services
        .admin
        .revoke_role(&admin.actor, other.actor.user_id, "admin")
        .await
        .unwrap();
    assert_eq!(user.roles, ["user"]);
}

#[tokio::test]
async fn revoking_a_role_nobody_manages_through_is_always_allowed() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;

    let user = fx
        .services
        .admin
        .revoke_role(&admin.actor, alice.actor.user_id, "user")
        .await
        .unwrap();

    assert!(user.roles.is_empty());
    let permissions = fx.db.with(|state| {
        state
            .user_roles
            .iter()
            .filter(|(user, _)| *user == alice.actor.user_id)
            .count()
    });
    assert_eq!(permissions, 0);
}

#[tokio::test]
async fn disabling_signs_the_user_out_and_enabling_restores_access() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.register("alice@example.com").await;
    let id = UserId::from_uuid(alice.me.user.id);

    let user = fx
        .services
        .admin
        .set_status(
            &admin.actor,
            id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap();
    assert!(user.disabled);
    assert!(
        fx.services
            .auth
            .authenticate(&alice.token, ClientInfo::default())
            .await
            .unwrap()
            .is_none()
    );

    let user = fx
        .services
        .admin
        .set_status(&admin.actor, id, application::admin::AccountStatus::Enabled)
        .await
        .unwrap();
    assert!(!user.disabled);
}

#[tokio::test]
async fn admins_cannot_disable_themselves_but_can_disable_other_admins() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let err = fx
        .services
        .admin
        .set_status(
            &admin.actor,
            admin.actor.user_id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "cannot_disable_self");

    let other = fx.admin("other@example.com").await;
    let user = fx
        .services
        .admin
        .set_status(
            &admin.actor,
            other.actor.user_id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap();
    assert!(user.disabled);
    assert!(
        fx.db
            .with(|state| !state.sessions.contains_key(&other.session.id()))
    );

    let err = fx
        .services
        .admin
        .set_status(
            &admin.actor,
            UserId::generate(),
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn disabled_managers_do_not_count_as_remaining_admins() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let other = fx.admin("other@example.com").await;
    fx.services
        .admin
        .set_status(
            &admin.actor,
            other.actor.user_id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap();

    let err = fx
        .services
        .admin
        .revoke_role(&admin.actor, admin.actor.user_id, "admin")
        .await
        .unwrap_err();

    assert_eq!(err.code(), "last_admin");
    assert!(fx.db.with(|state| {
        state
            .user_roles
            .contains(&(admin.actor.user_id, domain::rbac::RoleName::ADMIN))
    }));
}

#[tokio::test]
async fn admin_changes_need_a_recent_sign_in() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;
    let stale = application::actor::Actor {
        recently_authenticated: false,
        ..admin.actor.clone()
    };
    let target = alice.actor.user_id;

    let results = [
        fx.services
            .admin
            .grant_role(&stale, target, "admin")
            .await
            .map(|_| ()),
        fx.services
            .admin
            .revoke_role(&stale, target, "user")
            .await
            .map(|_| ()),
        fx.services
            .admin
            .set_status(&stale, target, application::admin::AccountStatus::Disabled)
            .await
            .map(|_| ()),
    ];
    for result in results {
        assert_eq!(result.unwrap_err().code(), "reauth_required");
    }
    assert!(
        fx.db
            .with(|state| state.users[&target].disabled_at().is_none())
    );
}

#[tokio::test]
async fn managers_only_hand_out_and_act_on_what_they_hold() {
    use domain::rbac::{Permission, PermissionSet, Role, RoleName};

    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let moderator = fx.user("moderator@example.com").await;
    let alice = fx.user("alice@example.com").await;
    let role = RoleName::parse("moderator").unwrap();
    fx.db.with(|state| {
        state.roles.insert(
            role.clone(),
            Role {
                name: role.clone(),
                description: "Moderates users".to_owned(),
                permissions: [Permission::UsersRead, Permission::UsersManage]
                    .into_iter()
                    .collect::<PermissionSet>(),
            },
        );
        state
            .user_roles
            .insert((moderator.actor.user_id, role.clone()));
    });
    let mut permissions = moderator.actor.permissions;
    permissions.insert(Permission::UsersRead);
    permissions.insert(Permission::UsersManage);
    let moderator = application::actor::Actor {
        permissions,
        ..moderator.actor
    };

    let forbidden = [
        fx.services
            .admin
            .grant_role(&moderator, moderator.user_id, "admin")
            .await
            .map(|_| ()),
        fx.services
            .admin
            .revoke_role(&moderator, admin.actor.user_id, "admin")
            .await
            .map(|_| ()),
        fx.services
            .admin
            .set_status(
                &moderator,
                admin.actor.user_id,
                application::admin::AccountStatus::Disabled,
            )
            .await
            .map(|_| ()),
    ];
    for result in forbidden {
        assert_eq!(result.unwrap_err().code(), "forbidden");
    }

    let user = fx
        .services
        .admin
        .grant_role(&moderator, alice.actor.user_id, "moderator")
        .await
        .unwrap();
    assert_eq!(user.roles, ["moderator", "user"]);
    fx.services
        .admin
        .set_status(
            &moderator,
            alice.actor.user_id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap();
}
