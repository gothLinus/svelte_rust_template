use application::{
    AppError,
    auth::{
        Browser, Registered,
        dto::{ForgotPasswordRequest, LoginRequest, ResetPasswordRequest, VerifyEmailRequest},
    },
    dto::permission_names,
};
use domain::{
    i18n::Message, rbac::default_user_permissions, secret::Secret, session::ClientInfo,
    user::UserId,
};
use time::Duration;

use crate::support::{APP_URL, Fixture, Outcome, PASSWORD, register_request, secret};

fn login(email: &str, password: &str) -> LoginRequest {
    LoginRequest {
        identifier: email.to_owned(),
        password: secret(password),
    }
}

#[tokio::test]
async fn register_creates_a_user_with_the_default_role_and_signs_in() {
    let fx = Fixture::new();

    let signed_in = fx.register("Alice@Example.com").await;

    assert_eq!(signed_in.me.user.email, "alice@example.com");
    assert_eq!(signed_in.me.user.roles, ["user"]);
    assert_eq!(
        signed_in.me.permissions,
        permission_names(default_user_permissions())
    );
    assert!(!signed_in.me.user.email_verified);
    let auth = fx.authenticate(&signed_in.token).await;
    assert_eq!(auth.session.id(), signed_in.session.id());
}

#[tokio::test]
async fn register_mails_a_verification_link() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.subject, fx.text("mail-verify-email-subject"));
    assert!(
        mail.body
            .contains(&format!("{APP_URL}/verify-email#token="))
    );
}

#[tokio::test]
async fn register_rejects_a_taken_email_case_insensitively() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;

    let err = fx
        .services
        .auth
        .register(register_request("ALICE@example.com"), ClientInfo::default())
        .await
        .unwrap_err();

    assert_eq!(err.code(), "email_taken");
}

#[tokio::test]
async fn register_reports_every_invalid_field() {
    let fx = Fixture::new();
    let mut request = register_request("not-an-email");
    request.username = "x".to_owned();
    request.password = secret("short");

    let AppError::Validation(errors) = fx
        .services
        .auth
        .register(request, ClientInfo::default())
        .await
        .unwrap_err()
    else {
        panic!("expected a validation error");
    };

    let fields: Vec<(&str, &str)> = errors
        .fields()
        .iter()
        .map(|e| (e.field.as_str(), e.code.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            ("email", "invalid_email"),
            ("username", "too_short"),
            ("password", "too_short")
        ]
    );
    assert!(fx.db.with(|state| state.users.is_empty()));
}

#[tokio::test]
async fn with_required_verification_registration_does_not_sign_in() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);

    let outcome = fx
        .services
        .auth
        .register(register_request("alice@example.com"), ClientInfo::default())
        .await
        .unwrap();

    let Registered::VerificationPending { pending, browser } = outcome else {
        panic!("expected verification to be pending");
    };
    assert_eq!(pending.email, "alice@example.com");
    assert!(fx.db.with(|state| state.sessions.is_empty()));

    let err = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "email_not_verified");

    let token = fx.mail.token_for("alice@example.com");
    fx.services
        .auth
        .verify_email(
            VerifyEmailRequest {
                token: application::dto::SecretInput(token),
            },
            Browser {
                registration: Some(&browser),
                ..Browser::default()
            },
        )
        .await
        .unwrap();
    fx.services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
}

#[tokio::test]
async fn login_succeeds_with_the_right_password() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;

    let signed_in = fx
        .services
        .auth
        .login(
            login(" Alice@Example.com ", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();

    assert_eq!(signed_in.me.user.email, "alice@example.com");
}

#[tokio::test]
async fn unknown_email_and_wrong_password_look_the_same() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;
    let before = fx.hasher.verifications();

    let wrong_password = fx
        .services
        .auth
        .login(
            login("alice@example.com", "wrong password"),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    let unknown_email = fx
        .services
        .auth
        .login(
            login("bob@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    let malformed_email = fx
        .services
        .auth
        .login(login("bob", PASSWORD), ClientInfo::default(), None)
        .await
        .unwrap_err();

    for err in [&wrong_password, &unknown_email, &malformed_email] {
        assert_eq!(err.code(), "invalid_credentials");
        assert_eq!(err.message(), Message::new("error-invalid-credentials"));
    }
    // Every attempt paid for a password verification, known account or not.
    assert_eq!(fx.hasher.verifications() - before, 3);
}

#[tokio::test]
async fn disabled_accounts_cannot_sign_in() {
    let fx = Fixture::new();
    let alice = fx.register("alice@example.com").await;
    let id = UserId::from_uuid(alice.me.user.id);
    let admin = fx.admin("admin@example.com").await;
    fx.services
        .admin
        .set_status(
            &admin.actor,
            id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap();

    let err = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "account_disabled");

    let err = fx
        .services
        .auth
        .login(
            login("alice@example.com", "nope nope"),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_credentials");
}

#[tokio::test]
async fn login_revokes_the_session_it_replaces() {
    let fx = Fixture::new();
    let first = fx.register("alice@example.com").await;

    let second = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            Some(&first.token),
        )
        .await
        .unwrap()
        .signed_in();

    assert_ne!(first.token.expose(), second.token.expose());
    assert!(
        fx.services
            .auth
            .authenticate(&first.token)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fx.services
            .auth
            .authenticate(&second.token)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn sessions_expire_after_the_idle_timeout() {
    let fx = Fixture::new();
    let signed_in = fx.register("alice@example.com").await;

    fx.clock.advance(Duration::days(6));
    assert!(
        fx.services
            .auth
            .authenticate(&signed_in.token)
            .await
            .unwrap()
            .is_some()
    );

    fx.clock.advance(Duration::days(6));
    assert!(
        fx.services
            .auth
            .authenticate(&signed_in.token)
            .await
            .unwrap()
            .is_some()
    );

    fx.clock.advance(Duration::days(7));
    assert!(
        fx.services
            .auth
            .authenticate(&signed_in.token)
            .await
            .unwrap()
            .is_none()
    );
    assert!(fx.db.with(|state| state.sessions.is_empty()));
}

#[tokio::test]
async fn sessions_end_at_the_absolute_lifetime_however_active() {
    let fx = Fixture::new();
    let signed_in = fx.register("alice@example.com").await;

    for _ in 0..29 {
        fx.clock.advance(Duration::days(1));
        assert!(
            fx.services
                .auth
                .authenticate(&signed_in.token)
                .await
                .unwrap()
                .is_some()
        );
    }
    fx.clock.advance(Duration::days(1));
    assert!(
        fx.services
            .auth
            .authenticate(&signed_in.token)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn activity_is_written_at_most_once_per_interval() {
    let fx = Fixture::new();
    let signed_in = fx.register("alice@example.com").await;
    let id = signed_in.session.id();
    let last_seen = || fx.db.with(|state| state.sessions[&id].last_seen_at());
    let start = last_seen();

    fx.clock.advance(Duration::seconds(30));
    fx.authenticate(&signed_in.token).await;
    assert_eq!(last_seen(), start);

    fx.clock.advance(Duration::seconds(30));
    fx.authenticate(&signed_in.token).await;
    assert_eq!(last_seen(), start + Duration::minutes(1));
}

#[tokio::test]
async fn garbage_tokens_do_not_authenticate() {
    let fx = Fixture::new();
    for token in ["", "nope", &"x".repeat(5000)] {
        assert!(
            fx.services
                .auth
                .authenticate(&Secret::new(token))
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn a_pending_rotation_hands_out_a_new_token_once() {
    let fx = Fixture::new();
    let alice = fx.register("alice@example.com").await;
    let id = UserId::from_uuid(alice.me.user.id);
    let admin = fx.admin("admin@example.com").await;
    fx.services
        .admin
        .grant_role(&admin.actor, id, "admin")
        .await
        .unwrap();

    let rotated = fx.authenticate(&alice.token).await;
    let new_token = rotated.rotated_token.expect("no new token");
    assert!(rotated.actor.has(domain::rbac::Permission::UsersManage));

    assert!(
        fx.services
            .auth
            .authenticate(&alice.token)
            .await
            .unwrap()
            .is_none()
    );
    let again = fx.authenticate(&new_token).await;
    assert!(again.rotated_token.is_none());
}

#[tokio::test]
async fn logout_ends_one_session_and_logout_everywhere_ends_all() {
    let fx = Fixture::new();
    let first = fx.register("alice@example.com").await;
    let second = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
    let third = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();

    fx.services.auth.logout(&first.token).await.unwrap();
    fx.services.auth.logout(&first.token).await.unwrap();
    assert!(
        fx.services
            .auth
            .authenticate(&first.token)
            .await
            .unwrap()
            .is_none()
    );

    let actor = fx.authenticate(&second.token).await.actor;
    assert_eq!(fx.services.auth.logout_everywhere(&actor).await.unwrap(), 2);
    assert!(
        fx.services
            .auth
            .authenticate(&third.token)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn verification_links_work_once_and_expire() {
    let fx = Fixture::new();
    let alice = fx.register("alice@example.com").await;
    let token = fx.mail.token_for("alice@example.com");
    let verify = |token: &Secret| VerifyEmailRequest {
        token: application::dto::SecretInput(token.clone()),
    };

    fx.services
        .auth
        .verify_email(
            verify(&token),
            Browser {
                session: Some(&alice.token),
                ..Browser::default()
            },
        )
        .await
        .unwrap();
    let me = fx
        .services
        .account
        .me(&fx.authenticate(&alice.token).await.user)
        .await
        .unwrap();
    assert!(me.user.email_verified);

    let err = fx
        .services
        .auth
        .verify_email(verify(&token), Browser::default())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_token");

    let bob = fx.unverified_user("bob@example.com").await;
    let token = fx.mail.token_for("bob@example.com");
    fx.clock.advance(Duration::days(1));
    let err = fx
        .services
        .auth
        .verify_email(verify(&token), Browser::default())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_token");
    assert!(!bob.user.is_email_verified());
}

#[tokio::test]
async fn resending_verification_invalidates_the_old_link() {
    let fx = Fixture::new();
    let alice = fx.unverified_user("alice@example.com").await;
    let old = fx.mail.token_for("alice@example.com");

    fx.services
        .auth
        .resend_verification(&alice.actor)
        .await
        .unwrap();
    let new = fx.mail.token_for("alice@example.com");
    assert_ne!(old.expose(), new.expose());

    let request = |token: Secret| VerifyEmailRequest {
        token: application::dto::SecretInput(token),
    };
    assert!(matches!(
        fx.services
            .auth
            .verify_email(request(old), Browser::default())
            .await,
        Err(AppError::InvalidToken)
    ));
    fx.services
        .auth
        .verify_email(request(new), Browser::default())
        .await
        .unwrap();

    let err = fx
        .services
        .auth
        .resend_verification(&alice.actor)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "already_verified");
}

#[tokio::test]
async fn password_reset_for_an_unknown_address_succeeds_silently() {
    let fx = Fixture::new();
    fx.services
        .auth
        .request_password_reset(ForgotPasswordRequest {
            email: "nobody@example.com".to_owned(),
        })
        .await
        .unwrap();
    assert!(fx.mail.sent().is_empty());

    let err = fx
        .services
        .auth
        .request_password_reset(ForgotPasswordRequest {
            email: "nobody".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");
}

#[tokio::test]
async fn password_reset_sets_the_password_and_signs_out_everywhere() {
    let fx = Fixture::new();
    let alice = fx.register("alice@example.com").await;

    fx.services
        .auth
        .request_password_reset(ForgotPasswordRequest {
            email: "alice@example.com".to_owned(),
        })
        .await
        .unwrap();
    let token = fx.mail.token_for("alice@example.com");
    assert!(
        fx.mail
            .last_to("alice@example.com")
            .unwrap()
            .body
            .contains("/reset-password#token=")
    );

    let reset = |token: Secret, password: &str| ResetPasswordRequest {
        token: application::dto::SecretInput(token),
        password: secret(password),
    };
    fx.services
        .auth
        .reset_password(reset(token.clone(), "a brand new password"))
        .await
        .unwrap();

    assert!(
        fx.services
            .auth
            .authenticate(&alice.token)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fx.mail.last_to("alice@example.com").unwrap().subject,
        fx.text("mail-password-changed-subject")
    );
    fx.services
        .auth
        .login(
            login("alice@example.com", "a brand new password"),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
    let me = fx
        .db
        .with(|state| state.users.values().next().unwrap().is_email_verified());
    assert!(me);

    let err = fx
        .services
        .auth
        .reset_password(reset(token, "another new password"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_token");
}

#[tokio::test]
async fn reset_links_expire_and_need_a_valid_password() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;
    fx.services
        .auth
        .request_password_reset(ForgotPasswordRequest {
            email: "alice@example.com".to_owned(),
        })
        .await
        .unwrap();
    let token = fx.mail.token_for("alice@example.com");

    let err = fx
        .services
        .auth
        .reset_password(ResetPasswordRequest {
            token: application::dto::SecretInput(token.clone()),
            password: secret("short"),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    fx.clock.advance(Duration::minutes(31));
    let err = fx
        .services
        .auth
        .reset_password(ResetPasswordRequest {
            token: application::dto::SecretInput(token),
            password: secret("long enough password"),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_token");
}

#[tokio::test]
async fn storage_failures_become_internal_errors() {
    let fx = Fixture::new();
    fx.db.with(|state| state.broken = true);

    let err = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Internal(_)));
    assert_eq!(err.code(), "internal_error");
    assert_eq!(
        fx.services.health.ready().await,
        application::health::HealthStatus::Unavailable
    );
}

#[tokio::test]
async fn readiness_is_checked_at_most_once_a_second() {
    use application::health::HealthStatus;

    let fx = Fixture::new();
    assert_eq!(fx.services.health.ready().await, HealthStatus::Ok);

    // Within the second, probes reuse the answer instead of taking a connection each.
    fx.db.with(|state| state.broken = true);
    assert_eq!(fx.services.health.ready().await, HealthStatus::Ok);

    fx.clock.advance(Duration::seconds(1));
    assert_eq!(fx.services.health.ready().await, HealthStatus::Unavailable);
}

#[tokio::test]
async fn maintenance_deletes_expired_sessions_and_tokens() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;
    fx.register("bob@example.com").await;

    fx.clock.advance(Duration::days(8));
    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();

    assert_eq!(cleanup.sessions, 2);
    assert_eq!(cleanup.tokens, 2);
    assert!(
        fx.db
            .with(|state| state.sessions.is_empty() && state.tokens.is_empty())
    );
}

#[tokio::test]
async fn signing_in_rehashes_a_password_made_with_old_parameters() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.db.with(|state| {
        let user = state.users.get_mut(&alice.actor.user_id).unwrap();
        let old = domain::user::PasswordHash::new(format!("old-hashed:{PASSWORD}"));
        let mut parts = crate::support::memory::user_parts(user);
        parts.password_hash = Some(old);
        *user = domain::user::User::from_parts(parts);
    });

    fx.services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap();

    let hash = fx.db.with(|state| {
        state.users[&alice.actor.user_id]
            .password_hash()
            .unwrap()
            .as_str()
            .to_owned()
    });
    assert_eq!(hash, format!("hashed:{PASSWORD}"));
}

#[tokio::test]
async fn with_required_verification_only_verified_accounts_keep_their_sessions() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);
    let Registered::VerificationPending { browser, .. } = fx
        .services
        .auth
        .register(register_request("alice@example.com"), ClientInfo::default())
        .await
        .unwrap()
    else {
        panic!("expected verification to be pending");
    };
    fx.services
        .auth
        .verify_email(
            VerifyEmailRequest {
                token: application::dto::SecretInput(fx.mail.token_for("alice@example.com")),
            },
            Browser {
                registration: Some(&browser),
                ..Browser::default()
            },
        )
        .await
        .unwrap();
    let signed_in = fx
        .services
        .auth
        .login(
            login("alice@example.com", PASSWORD),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
    assert!(
        fx.services
            .auth
            .authenticate(&signed_in.token)
            .await
            .unwrap()
            .is_some()
    );

    let id = UserId::from_uuid(signed_in.me.user.id);
    fx.db.with(|state| {
        let mut parts = crate::support::memory::user_parts(&state.users[&id]);
        parts.email_verified_at = None;
        state
            .users
            .insert(id, domain::user::User::from_parts(parts));
    });
    assert!(
        fx.services
            .auth
            .authenticate(&signed_in.token)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn maintenance_enforces_retention_for_unverified_accounts_and_abandoned_setups() {
    let fx = Fixture::with(|settings| settings.unverified_account_ttl = Some(Duration::days(7)));
    let unverified = fx.unverified_user("alice@example.com").await;
    let verified = fx.user("bob@example.com").await;
    let admin = fx.unverified_user("carol@example.com").await;
    fx.db.with(|state| {
        state
            .user_roles
            .insert((admin.actor.user_id, domain::rbac::RoleName::ADMIN));
    });
    fx.services.mfa.start_totp(&verified.actor).await.unwrap();

    fx.clock.advance(Duration::days(6));
    let early = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(early.accounts, 0);
    assert!(fx.db.with(|state| state.totp.is_empty()));

    fx.clock.advance(Duration::days(2));
    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(cleanup.accounts, 1);
    fx.db.with(|state| {
        assert!(!state.users.contains_key(&unverified.actor.user_id));
        assert!(state.users.contains_key(&verified.actor.user_id));
        assert!(state.users.contains_key(&admin.actor.user_id));
    });
}

#[tokio::test]
async fn without_a_retention_period_unverified_accounts_stay() {
    let fx = Fixture::new();
    let alice = fx.unverified_user("alice@example.com").await;
    fx.clock.advance(Duration::days(400));
    assert_eq!(
        fx.services
            .maintenance
            .delete_expired()
            .await
            .unwrap()
            .accounts,
        0
    );
    assert!(
        fx.db
            .with(|state| state.users.contains_key(&alice.actor.user_id))
    );
}
