use std::{num::NonZeroU32, sync::Arc, time::Duration};

use api::{
    rate_limit::{Rate, Rates},
    router,
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use domain::i18n::{Locale, Message, Translator};
use http_body_util::BodyExt;
use proto::v1;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

use crate::support::{Options, TestApp, TestRequest, catalog_with_german, http_config};

fn german() -> Locale {
    Locale::parse("de").unwrap()
}

fn app_with_german(pool: PgPool) -> TestApp {
    TestApp::with(
        pool,
        Options {
            catalog: Some(catalog_with_german()),
            ..Options::default()
        },
    )
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn problems_are_written_in_the_requested_language(pool: PgPool) {
    let app = app_with_german(pool);

    let response = app
        .send(TestRequest::get("/api/v1/nothing-here").header("accept-language", "de"))
        .await;

    response.assert_problem(StatusCode::NOT_FOUND, "not_found");
    let expected = |id| app.catalog.translate(&german(), &Message::new(id));
    assert_eq!(response.body["detail"], expected("error-not-found"));
    assert_eq!(response.body["title"], expected("http-status-404"));
    // The catalog has German for these, and the English text is different.
    assert_ne!(response.body["detail"], app.text("error-not-found"));
    assert_eq!(response.header("content-language"), Some("de"));
    assert!(
        response
            .header("vary")
            .is_some_and(|vary| vary.to_lowercase().contains("accept-language"))
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_regional_or_weighted_preference_finds_the_language(pool: PgPool) {
    let app = app_with_german(pool);

    let response = app
        .send(
            TestRequest::get("/api/v1/nothing-here")
                .header("accept-language", "fr;q=0.9, de-AT;q=0.8, en;q=0.5"),
        )
        .await;

    assert_eq!(response.header("content-language"), Some("de"));
    assert_eq!(
        response.body["detail"],
        app.catalog
            .translate(&german(), &Message::new("error-not-found"))
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unsupported_or_missing_languages_fall_back_to_english(pool: PgPool) {
    let app = app_with_german(pool);

    for accept in [
        Some("fr"),
        Some("zz-Latn, ja;q=0.5"),
        Some("not a header"),
        None,
    ] {
        let mut request = TestRequest::get("/api/v1/nothing-here");
        if let Some(accept) = accept {
            request = request.header("accept-language", accept);
        }
        let response = app.send(request).await;

        assert_eq!(
            response.body["detail"],
            app.text("error-not-found"),
            "{accept:?}"
        );
        assert_eq!(
            response.header("content-language"),
            Some("en"),
            "{accept:?}"
        );
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_message_a_language_lacks_falls_back_to_english(pool: PgPool) {
    let app = app_with_german(pool);

    let response = app
        .send(TestRequest::put("/api/v1/auth/login").header("accept-language", "de"))
        .await;

    response.assert_problem(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
    assert_eq!(response.body["title"], app.text("http-status-405"));
    assert_eq!(
        response.body["detail"],
        app.catalog
            .translate(&german(), &Message::new("http-method-not-allowed"))
    );
    assert_eq!(response.header("content-language"), Some("de"));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn validation_messages_are_localized_field_by_field(pool: PgPool) {
    let app = app_with_german(pool);

    let response = app
        .send(
            TestRequest::post("/api/v1/auth/register")
                .header("accept-language", "de")
                .proto(&v1::RegisterRequest {
                    email: "nope".to_owned(),
                    username: String::new(),
                    password: "short".to_owned(),
                }),
        )
        .await;

    response.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    let say = |message: &Message| app.catalog.translate(&german(), message);
    assert_eq!(
        response.body["detail"],
        say(&Message::new("error-validation-failed"))
    );
    let errors = response.body["errors"].as_array().unwrap();
    let field =
        |name: &str| -> &Value { errors.iter().find(|error| error["field"] == name).unwrap() };
    assert_eq!(
        field("username")["message"],
        say(&Message::new("validation-required"))
    );
    assert_eq!(
        field("password")["message"],
        say(&Message::new("validation-password-too-short").arg("min", 8))
    );
    assert!(field("password")["message"].as_str().unwrap().contains('8'));
    assert_eq!(
        field("email")["message"],
        app.text("validation-invalid-email")
    );
    assert_eq!(field("email")["code"], "invalid_email");
    assert_eq!(field("password")["code"], "too_short");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn errors_from_middleware_and_extractors_are_localized_too(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            catalog: Some(catalog_with_german()),
            rates: Some(Rates {
                api_per_ip: Some(Rate::new(
                    NonZeroU32::new(1).unwrap(),
                    Duration::from_hours(2),
                )),
                ..Rates::default()
            }),
            ..Options::default()
        },
    );
    let say = |id| app.catalog.translate(&german(), &Message::new(id));

    let csrf = app
        .send(
            TestRequest::post("/api/v1/auth/login")
                .without_csrf_headers()
                .header("accept-language", "de"),
        )
        .await;
    csrf.assert_problem(StatusCode::FORBIDDEN, "csrf_rejected");
    assert_eq!(csrf.body["detail"], say("http-csrf-header-required"));

    let mut limited = app
        .send(TestRequest::get("/api/v1/me").header("accept-language", "de"))
        .await;
    for _ in 0..3 {
        if limited.status == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
        limited = app
            .send(TestRequest::get("/api/v1/me").header("accept-language", "de"))
            .await;
    }
    limited.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    assert_eq!(limited.body["detail"], say("http-rate-limited"));
    assert!(limited.header("retry-after").is_some());
    assert_eq!(limited.header("content-language"), Some("de"));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn success_responses_are_not_touched(pool: PgPool) {
    let app = app_with_german(pool);
    let token = app.register("alice@example.com").await;

    let me = app
        .send(
            TestRequest::get("/api/v1/me")
                .session(&token)
                .header("accept-language", "de"),
        )
        .await;

    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.header("content-language"), None);
    assert_eq!(
        me.decode::<v1::Me>().user.unwrap().email,
        "alice@example.com"
    );
}

async fn boom() -> &'static str {
    panic!("on purpose")
}

/// Timeouts and panics happen below the localizing layer, in `with_middleware` itself.
#[tokio::test]
async fn timeouts_and_panics_are_localized() {
    let options = Options::default();
    let mut config = http_config(&options);
    config.request_timeout = Duration::from_millis(50);
    let translator: Arc<dyn Translator> = catalog_with_german();
    let routes = Router::new()
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }),
        )
        .route("/boom", get(boom));
    let app = router::with_middleware(routes, &config, Arc::clone(&translator));
    let say = |id| translator.translate(&german(), &Message::new(id));

    let get = |uri: &str, language: &str| {
        Request::get(uri)
            .header("accept-language", language)
            .body(Body::empty())
            .unwrap()
    };
    for (uri, status, code, id) in [
        (
            "/slow",
            StatusCode::SERVICE_UNAVAILABLE,
            "timeout",
            "http-timeout",
        ),
        (
            "/boom",
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "error-internal",
        ),
    ] {
        let response = app.clone().oneshot(get(uri, "de")).await.unwrap();
        assert_eq!(response.status(), status, "{uri}");
        assert_eq!(response.headers()["content-language"], "de");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["code"], code);
        assert_eq!(body["detail"], say(id), "{uri}");

        let english = app.clone().oneshot(get(uri, "fr")).await.unwrap();
        assert_eq!(english.headers()["content-language"], "en");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn an_account_keeps_the_language_it_registered_in_and_can_change_it(pool: PgPool) {
    let app = TestApp::new(pool);
    let response = app
        .send(
            TestRequest::post("/api/v1/auth/register")
                .proto(&v1::RegisterRequest {
                    email: "alice@example.com".to_owned(),
                    username: "alice".to_owned(),
                    password: crate::support::PASSWORD.to_owned(),
                })
                .header("accept-language", "de-DE, en;q=0.5"),
        )
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
    let token = response.session_token().unwrap();
    assert_eq!(
        app.me(&token).await.user.unwrap().locale.as_deref(),
        Some("de")
    );

    let changed =
        app.send(TestRequest::put("/api/v1/me/locale").session(&token).proto(
            &v1::SetLocaleRequest {
                locale: Some("en".to_owned()),
            },
        ))
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{}", changed.text);
    assert_eq!(
        changed.decode::<v1::Me>().user.unwrap().locale.as_deref(),
        Some("en")
    );

    app.send(
        TestRequest::put("/api/v1/me/locale")
            .session(&token)
            .proto(&v1::SetLocaleRequest {
                locale: Some("xx".to_owned()),
            }),
    )
    .await
    .assert_field_error("locale", "unsupported_locale");
    app.send(TestRequest::put("/api/v1/me/locale").proto(&v1::SetLocaleRequest::default()))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn without_a_requested_language_an_account_has_none(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    assert!(app.me(&token).await.user.unwrap().locale.is_none());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn built_in_roles_are_described_in_the_requested_language(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;

    let roles = app
        .send(
            TestRequest::get("/api/v1/admin/roles")
                .session(&admin)
                .header("accept-language", "de"),
        )
        .await
        .decode::<v1::RoleList>()
        .roles;

    let described = roles.iter().find(|role| role.name == "admin").unwrap();
    assert_eq!(
        described.description,
        app.catalog
            .translate(&german(), &Message::new("role-admin-description"))
    );
    assert_ne!(described.description, app.text("role-admin-description"));
}
