use application::{
    account::dto::DeleteAccountRequest,
    auth::dto::{LoginRequest, ResetPasswordRequest},
};
use domain::session::{ClientInfo, SessionId};
use time::Duration;

use crate::support::{APP_URL, Fixture, Outcome, PASSWORD, secret};

async fn second_session(fx: &Fixture, email: &str) -> application::auth::SignedIn {
    fx.services
        .auth
        .login(
            LoginRequest {
                identifier: email.to_owned(),
                password: secret(PASSWORD),
            },
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in()
}

#[tokio::test]
async fn me_includes_roles_and_permissions() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let me = fx.services.account.me(&admin.user).await.unwrap();

    assert_eq!(me.user.roles, ["admin", "user"]);
    assert_eq!(
        me.permissions,
        application::dto::permission_names(domain::rbac::PermissionSet::all())
    );
}

#[tokio::test]
async fn changing_the_password_goes_through_a_mailed_link() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    fx.services
        .account
        .request_password_change(&alice.actor)
        .await
        .unwrap();

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.subject, fx.text("mail-password-reset-subject"));
    assert!(
        mail.body
            .contains(&format!("{APP_URL}/reset-password#token="))
    );

    let token = fx.mail.token_for("alice@example.com");
    fx.services
        .auth
        .reset_password(
            ResetPasswordRequest {
                token: application::dto::SecretInput(token),
                password: secret("a new password"),
            },
            ClientInfo::default(),
        )
        .await
        .unwrap();

    let login = |password: &str| LoginRequest {
        identifier: "alice@example.com".to_owned(),
        password: secret(password),
    };
    let old = fx
        .services
        .auth
        .login(login(PASSWORD), ClientInfo::default(), None)
        .await;
    assert_eq!(old.unwrap_err().code(), "invalid_credentials");
    fx.services
        .auth
        .login(login("a new password"), ClientInfo::default(), None)
        .await
        .unwrap()
        .signed_in();
}

#[tokio::test]
async fn sessions_lists_active_sessions_and_marks_the_current_one() {
    let fx = Fixture::new();
    let current = fx.user("alice@example.com").await;
    fx.clock.advance(Duration::minutes(5));
    second_session(&fx, "alice@example.com").await;

    let sessions = fx.services.account.sessions(&current.actor).await.unwrap();

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions.iter().filter(|s| s.current).count(), 1);
    assert_eq!(sessions[1].id, current.session.id().as_uuid());
    assert!(sessions[1].current);
}

#[tokio::test]
async fn revoking_a_session_only_works_for_your_own() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let other = second_session(&fx, "alice@example.com").await;
    let bob = fx.user("bob@example.com").await;

    let err = fx
        .services
        .account
        .revoke_session(&bob.actor, other.session.id())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");

    fx.services
        .account
        .revoke_session(&alice.actor, other.session.id())
        .await
        .unwrap();
    assert!(
        fx.services
            .auth
            .authenticate(&other.token, ClientInfo::default())
            .await
            .unwrap()
            .is_none()
    );

    let err = fx
        .services
        .account
        .revoke_session(&alice.actor, SessionId::generate())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn delete_account_needs_the_password() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let err = fx
        .services
        .account
        .delete_account(
            &alice.actor,
            DeleteAccountRequest {
                password: Some(secret("wrong password")),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    fx.services
        .account
        .delete_account(
            &alice.actor,
            DeleteAccountRequest {
                password: Some(secret(PASSWORD)),
            },
        )
        .await
        .unwrap();
    assert!(
        fx.db
            .with(|state| state.users.is_empty() && state.sessions.is_empty())
    );
}

#[tokio::test]
async fn the_last_admin_cannot_delete_their_account() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let err = fx
        .services
        .account
        .delete_account(
            &admin.actor,
            DeleteAccountRequest {
                password: Some(secret(PASSWORD)),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(err.code(), "last_admin");
    assert!(
        fx.db
            .with(|state| state.users.contains_key(&admin.actor.user_id))
    );

    fx.admin("second@example.com").await;
    fx.services
        .account
        .delete_account(
            &admin.actor,
            DeleteAccountRequest {
                password: Some(secret(PASSWORD)),
            },
        )
        .await
        .unwrap();
}
