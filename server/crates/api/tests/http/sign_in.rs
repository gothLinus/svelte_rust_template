use axum::http::StatusCode;
use domain::{identity::ProviderProfile, one_time_code::CODE_TTL, user_token::TokenPolicy};
use proto::v1;
use sqlx::PgPool;
use time::Duration;

use crate::support::{PASSWORD, TestApp, TestRequest, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn usernames_sign_in_and_login_needs_the_second_step(pool: PgPool) {
    let app = TestApp::new(pool);
    let response = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "alice@example.com".to_owned(),
                username: "Alice".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
    let me: v1::Me = response.decode();
    assert_eq!(user(&me).username, "alice");
    assert!(user(&me).has_password);
    let token = response.session_token().unwrap();
    app.verify_email("alice@example.com").await;

    let setup = app
        .send(TestRequest::post("/api/v1/me/mfa/totp").session(&token))
        .await;
    assert_eq!(setup.status, StatusCode::OK, "{}", setup.text);
    let secret = crate::support::base32_decode(&setup.decode::<v1::TotpSetup>().secret);
    let confirmed = app
        .send(
            TestRequest::post("/api/v1/me/mfa/totp/confirm")
                .session(&token)
                .proto(&v1::CodeRequest {
                    code: app.totp(&secret),
                }),
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.text);
    let codes = confirmed
        .decode::<v1::SecondFactorAdded>()
        .recovery_codes
        .unwrap()
        .codes;
    assert_eq!(codes.len(), 10);

    let first = app.login("alice", PASSWORD).await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{}", first.text);
    assert_eq!(
        first
            .decode::<v1::MfaChallenge>()
            .methods()
            .collect::<Vec<_>>(),
        [v1::MfaMethod::Totp, v1::MfaMethod::RecoveryCode]
    );
    assert!(first.session_token().is_none());
    let attempt = first.cookie("mfa").unwrap();

    let pending = app
        .send(TestRequest::get("/api/v1/auth/mfa").cookie("mfa", &attempt))
        .await;
    assert_eq!(
        pending.decode::<v1::MfaChallenge>().methods().next(),
        Some(v1::MfaMethod::Totp)
    );

    app.clock.advance(Duration::seconds(30));
    let second = app
        .send(
            TestRequest::post("/api/v1/auth/mfa/totp")
                .cookie("mfa", &attempt)
                .proto(&v1::CodeRequest {
                    code: app.totp(&secret),
                }),
        )
        .await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text);
    assert_eq!(user(&second.decode()).email, "alice@example.com");
    assert_eq!(second.cookie("mfa").as_deref(), Some(""));
    app.me(&second.session_token().unwrap()).await;

    let expired = app
        .send(
            TestRequest::post("/api/v1/auth/mfa/totp").proto(&v1::CodeRequest {
                code: "123456".to_owned(),
            }),
        )
        .await;
    expired.assert_problem(StatusCode::CONFLICT, "mfa_expired");

    let security = app
        .send(TestRequest::get("/api/v1/me/security").session(&token))
        .await;
    let security: v1::SecurityOverview = security.decode();
    assert!(security.mfa_enabled);
    assert!(security.totp_enabled);
    assert_eq!(security.recovery_codes_remaining, 10);
}

fn email_code(email: &str) -> v1::EmailCodeRequest {
    v1::EmailCodeRequest {
        email: email.to_owned(),
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn an_emailed_code_or_link_signs_in(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;

    let request = app
        .send(TestRequest::post("/api/v1/auth/email-code").proto(&email_code("alice@example.com")))
        .await;
    assert_eq!(request.status, StatusCode::ACCEPTED);
    let unknown = app
        .send(TestRequest::post("/api/v1/auth/email-code").proto(&email_code("nobody@example.com")))
        .await;
    assert_eq!(unknown.status, StatusCode::ACCEPTED);

    let code = app.mailed_code("alice@example.com");
    let wrong = app
        .send(TestRequest::post("/api/v1/auth/email-code/verify").proto(
            &v1::VerifyEmailCodeRequest {
                email: "alice@example.com".to_owned(),
                code: "000000".to_owned(),
            },
        ))
        .await;
    wrong.assert_invalid_code();
    let signed_in = app
        .send(TestRequest::post("/api/v1/auth/email-code/verify").proto(
            &v1::VerifyEmailCodeRequest {
                email: "alice@example.com".to_owned(),
                code,
            },
        ))
        .await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.text);
    assert!(user(&signed_in.decode()).email_verified);

    app.send(TestRequest::post("/api/v1/auth/email-code").proto(&email_code("alice@example.com")))
        .await;
    let link = app
        .send(
            TestRequest::post("/api/v1/auth/magic-link").proto(&v1::MagicLinkRequest {
                token: app.mailed_token("alice@example.com"),
            }),
        )
        .await;
    assert_eq!(link.status, StatusCode::OK, "{}", link.text);
    assert!(link.session_token().is_some());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn phone_numbers_are_verified_by_text_and_then_sign_in(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;

    let added =
        app.send(TestRequest::post("/api/v1/me/phone").session(&token).proto(
            &v1::AddPhoneRequest {
                phone: "+49 170 1234567".to_owned(),
                channel: v1::TextChannel::Sms.into(),
            },
        ))
        .await;
    assert_eq!(added.status, StatusCode::ACCEPTED, "{}", added.text);
    let code = app.texts.last_code_to("+491701234567").unwrap();
    let verified = app
        .send(
            TestRequest::post("/api/v1/me/phone/verify")
                .session(&token)
                .proto(&v1::CodeRequest { code }),
        )
        .await;
    assert_eq!(verified.status, StatusCode::OK, "{}", verified.text);
    let me: v1::Me = verified.decode();
    assert_eq!(user(&me).phone.as_deref(), Some("+491701234567"));
    assert!(user(&me).phone_verified);

    assert_eq!(
        app.login("+491701234567", PASSWORD).await.status,
        StatusCode::OK
    );

    app.send(
        TestRequest::post("/api/v1/auth/phone-code").proto(&v1::PhoneCodeRequest {
            phone: "+491701234567".to_owned(),
            channel: v1::TextChannel::Whatsapp.into(),
        }),
    )
    .await;
    let code = app.texts.last_code_to("+491701234567").unwrap();
    let signed_in = app
        .send(TestRequest::post("/api/v1/auth/phone-code/verify").proto(
            &v1::VerifyPhoneCodeRequest {
                phone: "+491701234567".to_owned(),
                code,
            },
        ))
        .await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.text);

    let methods = app.send(TestRequest::get("/api/v1/auth/methods")).await;
    let policy = TokenPolicy::default();
    let whole = |units: i64| u32::try_from(units).unwrap();
    assert_eq!(
        methods.decode::<v1::AuthMethods>(),
        v1::AuthMethods {
            app_name: "Acme".to_owned(),
            providers: vec![v1::OAuthProvider {
                id: "test".to_owned(),
                name: "Test Provider".to_owned(),
            }],
            text_channels: vec![
                v1::TextChannel::Sms.into(),
                v1::TextChannel::Whatsapp.into(),
            ],
            // The test app's policy is the default one; each in the unit its mail uses.
            lifetimes: Some(v1::LinkLifetimes {
                sign_in_minutes: whole(policy.magic_link_ttl.whole_minutes()),
                text_code_minutes: whole(CODE_TTL.whole_minutes()),
                password_reset_minutes: whole(policy.password_reset_ttl.whole_minutes()),
                email_verification_hours: whole(policy.email_verification_ttl.whole_hours()),
                email_change_hours: whole(policy.email_change_ttl.whole_hours()),
            }),
        }
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn changing_the_email_address(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    let elsewhere = app
        .login("alice@example.com", PASSWORD)
        .await
        .session_token()
        .unwrap();

    let response = app
        .send(TestRequest::post("/api/v1/me/email").session(&token).proto(
            &v1::ChangeEmailRequest {
                email: "alice@new.example".to_owned(),
            },
        ))
        .await;
    assert_eq!(response.status, StatusCode::ACCEPTED, "{}", response.text);
    let notice = app.mail.last_to("alice@example.com").unwrap();
    assert_eq!(notice.template, "email_change_requested");
    assert!(notice.body.contains("/cancel-email-change#token="));

    let confirm = app
        .send(
            TestRequest::post("/api/v1/auth/confirm-email")
                .session(&token)
                .proto(&v1::ConfirmEmailRequest {
                    token: app.mailed_token("alice@new.example"),
                }),
        )
        .await;
    assert_eq!(confirm.status, StatusCode::NO_CONTENT, "{}", confirm.text);
    let me = app.me(&token).await;
    assert_eq!(user(&me).email, "alice@new.example");
    assert!(user(&me).email_verified);
    app.send(TestRequest::get("/api/v1/me").session(&elsewhere))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_old_address_can_undo_an_email_change(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    app.send(TestRequest::post("/api/v1/me/email").session(&token).proto(
        &v1::ChangeEmailRequest {
            email: "attacker@evil.example".to_owned(),
        },
    ))
    .await;
    let cancel = app.mailed_token("alice@example.com");
    app.send(
        TestRequest::post("/api/v1/auth/confirm-email").proto(&v1::ConfirmEmailRequest {
            token: app.mailed_token("attacker@evil.example"),
        }),
    )
    .await;

    let undone = app
        .send(TestRequest::post("/api/v1/auth/cancel-email-change").proto(
            &v1::CancelEmailChangeRequest {
                token: cancel.clone(),
            },
        ))
        .await;
    assert_eq!(undone.status, StatusCode::NO_CONTENT, "{}", undone.text);
    app.send(TestRequest::get("/api/v1/me").session(&token))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    let signed_in = app.login("alice@example.com", PASSWORD).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.text);
    assert_eq!(
        app.mail.last_to("alice@example.com").unwrap().template,
        "email_change_cancelled"
    );
    app.send(
        TestRequest::post("/api/v1/auth/cancel-email-change")
            .proto(&v1::CancelEmailChangeRequest { token: cancel }),
    )
    .await
    .assert_problem(StatusCode::BAD_REQUEST, "invalid_token");
}

fn location(response: &crate::support::TestResponse) -> &str {
    response.header("location").unwrap()
}

async fn start(app: &TestApp, path: &str, session: Option<&str>) -> (String, String) {
    let mut request = TestRequest::get(path);
    if let Some(session) = session {
        request = request.session(session);
    }
    let response = app.send(request).await;
    assert_eq!(response.status, StatusCode::SEE_OTHER, "{}", response.text);
    let url = location(&response);
    assert!(
        url.starts_with("https://provider.test/authorize?state="),
        "{url}"
    );
    let state = url
        .split("state=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_owned();
    let cookie = response.cookie("oauth").unwrap();
    assert_eq!(state, cookie);
    (state, cookie)
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn social_sign_up_and_sign_in_redirect_with_a_session(pool: PgPool) {
    let app = TestApp::new(pool);
    app.providers.add_account(
        "code-1",
        ProviderProfile {
            subject: "123".to_owned(),
            email: Some("carol@example.com".to_owned()),
            email_verified: true,
            name: Some("Carol".to_owned()),
        },
    );

    let (state, cookie) = start(&app, "/api/v1/auth/oauth/test?redirectTo=/notes", None).await;
    let callback = app
        .send(
            TestRequest::get(&format!(
                "/api/v1/auth/oauth/test/callback?code=code-1&state={state}"
            ))
            .cookie("oauth", &cookie),
        )
        .await;
    assert_eq!(callback.status, StatusCode::SEE_OTHER, "{}", callback.text);
    assert_eq!(location(&callback), "/notes");
    let token = callback.session_token().unwrap();
    let me = app.me(&token).await;
    assert_eq!(user(&me).email, "carol@example.com");
    assert!(!user(&me).has_password);

    let (_, verifier, _) = app.providers.exchanges().pop().unwrap();
    assert_eq!(verifier.len(), 43);

    let (state, _) = start(&app, "/api/v1/auth/oauth/test", None).await;
    let refused = app
        .send(TestRequest::get(&format!(
            "/api/v1/auth/oauth/test/callback?code=code-1&state={state}"
        )))
        .await;
    assert_eq!(location(&refused), "/login?error=oauth_state_invalid");

    let unknown = app.send(TestRequest::get("/api/v1/auth/oauth/nope")).await;
    assert_eq!(location(&unknown), "/login?error=not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn apple_style_form_posts_bounce_to_the_get_callback(pool: PgPool) {
    let app = TestApp::new(pool);

    let response = app
        .send(
            TestRequest::post("/api/v1/auth/oauth/test/callback")
                .without_csrf_headers()
                .header("origin", "https://appleid.apple.com")
                .body(
                    "application/x-www-form-urlencoded",
                    "code=abc&state=xyz&user=%7B%7D",
                ),
        )
        .await;
    assert_eq!(response.status, StatusCode::SEE_OTHER, "{}", response.text);
    assert_eq!(
        location(&response),
        "/api/v1/auth/oauth/test/callback?state=xyz&code=abc"
    );

    let unknown = app
        .send(
            TestRequest::post("/api/v1/auth/oauth/%2F%2Fevil.test/callback")
                .without_csrf_headers()
                .header("origin", "https://appleid.apple.com")
                .body("application/x-www-form-urlencoded", "code=abc&state=xyz"),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::SEE_OTHER, "{}", unknown.text);
    assert_eq!(location(&unknown), "/login?error=not_found");

    let other = app
        .send(
            TestRequest::post("/api/v1/auth/logout")
                .without_csrf_headers()
                .header("origin", "https://appleid.apple.com"),
        )
        .await;
    other.assert_problem(StatusCode::FORBIDDEN, "csrf_rejected");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn linking_a_social_account_from_the_settings(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    app.providers.add_account(
        "code-a",
        ProviderProfile {
            subject: "alice-at-provider".to_owned(),
            email: Some("alice@example.com".to_owned()),
            email_verified: true,
            name: None,
        },
    );

    let (state, cookie) = start(&app, "/api/v1/auth/oauth/test", None).await;
    let refused = app
        .send(
            TestRequest::get(&format!(
                "/api/v1/auth/oauth/test/callback?code=code-a&state={state}"
            ))
            .cookie("oauth", &cookie),
        )
        .await;
    assert_eq!(location(&refused), "/login?error=email_in_use");

    let (state, cookie) = start(&app, "/api/v1/auth/oauth/test/link", Some(&token)).await;
    let linked = app
        .send(
            TestRequest::get(&format!(
                "/api/v1/auth/oauth/test/callback?code=code-a&state={state}"
            ))
            .cookie("oauth", &cookie)
            .session(&token),
        )
        .await;
    assert_eq!(location(&linked), "/settings/security?linked=test");

    let accounts = app
        .send(TestRequest::get("/api/v1/me/linked-accounts").session(&token))
        .await;
    let accounts = accounts.decode::<v1::LinkedAccountList>().accounts;
    assert_eq!(accounts[0].provider, "test");
    assert_eq!(accounts[0].provider_name, "Test Provider");
    assert_eq!(accounts[0].email.as_deref(), Some("alice@example.com"));

    let unlinked = app
        .send(TestRequest::delete("/api/v1/me/linked-accounts/test").session(&token))
        .await;
    assert_eq!(unlinked.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn passkey_options_describe_the_ceremony(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;

    let creation = app
        .send(TestRequest::post("/api/v1/me/passkeys/options").session(&token))
        .await;
    assert_eq!(creation.status, StatusCode::OK, "{}", creation.text);
    let creation: v1::PasskeyCreationOptions = creation.decode();
    assert!(uuid::Uuid::try_parse(&creation.challenge_id).is_ok());
    let options = creation.public_key.unwrap();
    assert_eq!(options.rp.unwrap().id, "localhost");
    assert_eq!(options.attestation, "none");
    assert_eq!(options.algorithms[1], -7);
    assert_eq!(options.challenge.len(), 32);
    let passkey_user = options.user.unwrap();
    assert_eq!(passkey_user.id.len(), 16);
    assert_eq!(passkey_user.name, "alice");

    let request = app
        .send(TestRequest::post("/api/v1/auth/passkeys/options"))
        .await;
    let request: v1::PasskeyRequestOptions = request.decode();
    let public_key = request.public_key.unwrap();
    assert_eq!(public_key.user_verification, "required");
    assert!(public_key.allow_credentials.is_empty());
    assert_eq!(public_key.challenge.len(), 32);

    let forged = app
        .send(TestRequest::post("/api/v1/auth/passkeys/login").proto(
            &v1::PasskeyAssertionRequest {
                challenge_id: request.challenge_id,
                credential_id: vec![0; 3],
                client_data_json: b"{}".to_vec(),
                authenticator_data: vec![0; 3],
                signature: vec![0; 3],
                user_handle: None,
            },
        ))
        .await;
    forged.assert_problem(StatusCode::BAD_REQUEST, "invalid_passkey");

    let list = app
        .send(TestRequest::get("/api/v1/me/passkeys").session(&token))
        .await;
    assert_eq!(list.decode::<v1::PasskeyList>(), v1::PasskeyList::default());
}
