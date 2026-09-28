use std::{num::NonZeroU32, time::Duration};

use api::rate_limit::{Rate, Rates, account_key, api_ip_bucket, ip_bucket};
use axum::http::StatusCode;
use infrastructure::config::RateLimitStore;
use proto::v1;
use sqlx::PgPool;

use crate::support::{Options, PASSWORD, TestApp, TestRequest};

fn login_as(identifier: &str, password: &str, ip: &str) -> TestRequest {
    TestRequest::post("/api/v1/auth/login")
        .with_peer(ip)
        .proto(&v1::LoginRequest {
            identifier: identifier.to_owned(),
            password: password.to_owned(),
        })
}

fn right_login(identifier: &str, ip: &str) -> TestRequest {
    login_as(identifier, PASSWORD, ip)
}

fn tight() -> Rate {
    Rate::new(NonZeroU32::new(2).unwrap(), Duration::from_hours(2))
}

fn generous() -> Rate {
    Rate::new(NonZeroU32::new(1000).unwrap(), Duration::from_secs(60))
}

fn app(pool: PgPool, rates: &Rates) -> TestApp {
    TestApp::with(
        pool,
        Options {
            rates: Some(*rates),
            ..Options::default()
        },
    )
}

fn all(rate: Rate) -> Rates {
    Rates {
        api_per_ip: None,
        login_per_ip: rate,
        login_per_account: rate,
        login_per_account_total: rate,
        register_per_ip: rate,
        password_reset_per_ip: rate,
        password_reset_per_account: rate,
        verification_per_ip: rate,
        verification_per_account: rate,
        send_code_per_ip: rate,
        send_code_per_account: rate,
        check_code_per_ip: rate,
        check_code_per_account: rate,
        ceremony_per_ip: rate,
        report_per_ip: rate,
    }
}

fn login(email: &str, ip: &str) -> TestRequest {
    login_as(email, "wrong password", ip)
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn login_is_limited_per_ip(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            login_per_ip: tight(),
            ..all(generous())
        },
    );

    for n in 0..2 {
        let response = app
            .send(login(&format!("user{n}@example.com"), "198.51.100.1"))
            .await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
    let limited = app.send(login("user9@example.com", "198.51.100.1")).await;

    limited.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    let retry_after: u64 = limited.header("retry-after").unwrap().parse().unwrap();
    assert!((3000..=3600).contains(&retry_after), "{retry_after}");

    let other = app.send(login("user9@example.com", "198.51.100.2")).await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn limits_work_with_buckets_in_postgres(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            rates: Some(all(tight())),
            rate_limit_store: RateLimitStore::Postgres,
            ..Options::default()
        },
    );
    app.register("alice@example.com").await;

    for _ in 0..2 {
        let response = app
            .send(right_login("alice@example.com", "198.51.100.1"))
            .await;
        assert_eq!(response.status, StatusCode::OK);
    }
    app.send(login("alice@example.com", "198.51.100.2")).await;
    app.send(login("alice@example.com", "198.51.100.3")).await;
    app.send(login("alice@example.com", "198.51.100.4"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn login_is_limited_per_account_across_ips(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            login_per_account_total: tight(),
            ..all(generous())
        },
    );
    app.register("alice@example.com").await;

    app.send(login("alice@example.com", "198.51.100.1")).await;
    app.send(login("ALICE@example.com", "198.51.100.2")).await;
    let limited = app
        .send(right_login("alice@example.com", "198.51.100.3"))
        .await;

    limited.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    assert_eq!(
        app.send(login("bob@example.com", "198.51.100.3"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn registration_and_password_reset_are_limited(pool: PgPool) {
    let app = app(pool, &all(tight()));

    for n in 0..2 {
        app.register(&format!("user{n}@example.com")).await;
    }
    app.send(
        TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
            email: "user3@example.com".to_owned(),
            username: "user3".to_owned(),
            password: PASSWORD.to_owned(),
        }),
    )
    .await
    .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");

    let forgot = |ip: &str| {
        TestRequest::post("/api/v1/auth/forgot-password")
            .with_peer(ip)
            .proto(&v1::ForgotPasswordRequest {
                email: "user0@example.com".to_owned(),
            })
    };
    assert_eq!(
        app.send(forgot("198.51.100.1")).await.status,
        StatusCode::ACCEPTED
    );
    assert_eq!(
        app.send(forgot("198.51.100.2")).await.status,
        StatusCode::ACCEPTED
    );
    app.send(forgot("198.51.100.3"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn limits_are_off_when_disabled(pool: PgPool) {
    let app = TestApp::new(pool);
    for _ in 0..30 {
        let response = app.send(login("alice@example.com", "198.51.100.1")).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn only_failed_logins_count_against_the_account(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            login_per_account: tight(),
            ..all(generous())
        },
    );
    app.register("alice@example.com").await;

    for _ in 0..5 {
        let response = app
            .send(right_login("alice@example.com", "198.51.100.1"))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.text);
    }
    for _ in 0..2 {
        let response = app.send(login("alice@example.com", "203.0.113.5")).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
    app.send(right_login("alice@example.com", "203.0.113.5"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    let owner = app
        .send(right_login("alice@example.com", "198.51.100.1"))
        .await;
    assert_eq!(owner.status, StatusCode::OK, "{}", owner.text);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn every_spelling_of_an_account_shares_one_budget(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            login_per_account_total: tight(),
            ..all(generous())
        },
    );
    app.register("alice@example.com").await;
    app.set_phone("alice@example.com", "+491701234567").await;

    for (identifier, ip) in [
        ("+49 170 1234567", "198.51.100.1"),
        ("+49(170)123-4567", "198.51.100.2"),
    ] {
        let response = app.send(login(identifier, ip)).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED, "{identifier}");
    }
    for identifier in ["+491701234567", "alice@example.com", "ALICE"] {
        app.send(right_login(identifier, "198.51.100.3"))
            .await
            .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn codes_sent_to_a_number_are_limited_in_any_spelling(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            send_code_per_account: tight(),
            ..all(generous())
        },
    );
    let phone_code = |phone: &str, ip: &str| {
        TestRequest::post("/api/v1/auth/phone-code")
            .with_peer(ip)
            .proto(&v1::PhoneCodeRequest {
                phone: phone.to_owned(),
                channel: v1::TextChannel::Sms.into(),
            })
    };
    for (phone, ip) in [
        ("+49 170 1234567", "198.51.100.1"),
        ("+49.170.1234567", "198.51.100.2"),
    ] {
        assert_eq!(
            app.send(phone_code(phone, ip)).await.status,
            StatusCode::ACCEPTED
        );
    }
    app.send(phone_code("+49(170)1234567", "198.51.100.3"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn ipv6_clients_are_limited_per_64(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            login_per_ip: tight(),
            ..all(generous())
        },
    );
    for ip in ["2001:db8:0:1::1", "2001:db8:0:1:ffff:ffff:ffff:ffff"] {
        let response = app.send(login("user@example.com", ip)).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
    app.send(login("user@example.com", "2001:db8:0:1:1234::9"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    assert_eq!(
        app.send(login("user@example.com", "2001:db8:0:2::1"))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_second_step_is_limited_per_account_across_attempts_and_ips(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            check_code_per_account: tight(),
            ..all(generous())
        },
    );
    let token = app.register_verified("alice@example.com").await;
    app.enable_totp(&token).await;

    let guess = |attempt: &str, ip: &str| {
        TestRequest::post("/api/v1/auth/mfa/totp")
            .with_peer(ip)
            .cookie("mfa", attempt)
            .proto(&v1::CodeRequest {
                code: "000000".to_owned(),
            })
    };
    let attempt = app
        .login("alice@example.com", PASSWORD)
        .await
        .cookie("mfa")
        .unwrap();
    app.send(guess(&attempt, "198.51.100.1"))
        .await
        .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    let attempt = app
        .login("alice@example.com", PASSWORD)
        .await
        .cookie("mfa")
        .unwrap();
    app.send(guess(&attempt, "198.51.100.9"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn adding_a_phone_is_limited_per_destination_and_per_ip(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            send_code_per_account: tight(),
            ..all(generous())
        },
    );
    let add = |token: &str, phone: &str, ip: &str| {
        TestRequest::post("/api/v1/me/phone")
            .session(token)
            .with_peer(ip)
            .proto(&v1::AddPhoneRequest {
                phone: phone.to_owned(),
                channel: v1::TextChannel::Sms.into(),
            })
    };
    let mut tokens = Vec::new();
    for n in 0..3 {
        let email = format!("user{n}@example.com");
        tokens.push(app.register(&email).await);
        app.verify_email(&email).await;
    }
    for (n, token) in tokens.iter().take(2).enumerate() {
        let response = app
            .send(add(token, "+44 909 8790000", &format!("198.51.100.{n}")))
            .await;
        assert_eq!(response.status, StatusCode::ACCEPTED, "{}", response.text);
    }
    app.send(add(&tokens[2], "+449098790000", "198.51.100.9"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");

    let app = TestApp::with(
        app.pool.clone(),
        Options {
            rates: Some(Rates {
                send_code_per_ip: tight(),
                ..all(generous())
            }),
            ..Options::default()
        },
    );
    for (n, token) in tokens.iter().take(2).enumerate() {
        let response = app
            .send(add(token, &format!("+4915112345{n:02}"), "203.0.113.1"))
            .await;
        assert_eq!(response.status, StatusCode::ACCEPTED, "{}", response.text);
    }
    app.send(add(&tokens[2], "+491511234599", "203.0.113.1"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn every_api_request_is_limited_per_ip(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            api_per_ip: Some(tight()),
            ..all(generous())
        },
    );
    let me = |ip: &str| TestRequest::get("/api/v1/me").with_peer(ip);

    for _ in 0..2 {
        let response = app.send(me("198.51.100.1")).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
    let limited = app.send(me("198.51.100.1")).await;
    limited.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    assert!(limited.header("retry-after").is_some());
    app.send(login("alice@example.com", "198.51.100.1"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");

    let health = app
        .send(TestRequest::get("/health/live").with_peer("198.51.100.1"))
        .await;
    assert_eq!(health.status, StatusCode::OK);
    let other = app.send(me("198.51.100.2")).await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_api_limit_can_be_turned_off(pool: PgPool) {
    let app = app(pool, &all(tight()));
    for _ in 0..5 {
        let response = app
            .send(TestRequest::get("/api/v1/me").with_peer("198.51.100.1"))
            .await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn huge_identifiers_are_refused_without_growing_the_buckets(pool: PgPool) {
    let app = app(pool, &all(generous()));
    let huge = "a".repeat(60 * 1024);
    let response = app.send(login(&huge, "198.51.100.1")).await;
    assert_eq!(
        response.status,
        StatusCode::UNAUTHORIZED,
        "{}",
        response.text
    );
    assert!(account_key(&huge).len() < 100);
}

#[test]
fn account_keys_are_canonical() {
    assert_eq!(
        account_key("+49 170 1234567"),
        account_key("+49(170)123-4567")
    );
    assert_eq!(account_key("+49 170 1234567"), "phone:+491701234567");
    assert_eq!(
        account_key(" Alice@Example.com "),
        account_key("alice@example.com")
    );
    assert_eq!(account_key("Alice"), account_key("alice"));
    assert_ne!(account_key("alice"), account_key("alice@example.com"));
    assert!(account_key(&"x@".repeat(30_000)).chars().count() <= 4 + 64);
}

#[test]
fn ip_buckets_group_ipv6_by_64_and_unmap_ipv4() {
    let bucket = |ip: &str| ip_bucket(ip.parse().unwrap());
    assert_eq!(
        bucket("2001:db8:1:2:aaaa::1"),
        bucket("2001:db8:1:2:ffff:1:2:3")
    );
    assert_ne!(bucket("2001:db8:1:2::1"), bucket("2001:db8:1:3::1"));
    assert_eq!(bucket("::ffff:198.51.100.7"), bucket("198.51.100.7"));
    assert_eq!(bucket("198.51.100.7").to_string(), "198.51.100.7");

    let api = |ip: &str| api_ip_bucket(ip.parse().unwrap());
    assert_eq!(api("2001:db8:1:2ff::1"), api("2001:db8:1:200::1"));
    assert_ne!(api("2001:db8:1:200::1"), api("2001:db8:1:300::1"));
    assert_eq!(api("::ffff:198.51.100.7").to_string(), "198.51.100.7");
}

/// Every registration attempt mails the address, so rotating IPs must not let anyone flood one
/// inbox with them.
#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn registration_is_limited_per_address(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            rates: Some(Rates {
                send_code_per_account: tight(),
                ..all(generous())
            }),
            require_email_verification: true,
            ..Options::default()
        },
    );
    let register = |email: &str, ip: &str| {
        TestRequest::post("/api/v1/auth/register")
            .with_peer(ip)
            .proto(&v1::RegisterRequest {
                email: email.to_owned(),
                username: format!("u{}", ip.replace('.', "")),
                password: PASSWORD.to_owned(),
            })
    };

    for ip in ["198.51.100.1", "198.51.100.2"] {
        let response = app.send(register("victim@example.com", ip)).await;
        assert_eq!(response.status, StatusCode::ACCEPTED, "{}", response.text);
    }
    app.send(register("Victim@Example.com", "198.51.100.3"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    let mailed = app
        .mail
        .sent()
        .iter()
        .filter(|mail| mail.to.as_str() == "victim@example.com")
        .count();
    assert_eq!(mailed, 2);

    let other = app
        .send(register("other@example.com", "198.51.100.3"))
        .await;
    assert_eq!(other.status, StatusCode::ACCEPTED, "{}", other.text);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_owners_known_browser_is_exempt_from_the_account_ceiling(pool: PgPool) {
    let app = app(
        pool,
        &Rates {
            login_per_account_total: tight(),
            ..all(generous())
        },
    );
    let registered = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "alice@example.com".to_owned(),
                username: "alice".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;
    let device = registered.cookie("device").expect("the browser is marked");

    app.send(login("alice@example.com", "198.51.100.1")).await;
    app.send(login("alice@example.com", "198.51.100.2")).await;
    app.send(right_login("alice@example.com", "198.51.100.3"))
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");

    let owner = app
        .send(right_login("alice@example.com", "203.0.113.1").cookie("device", &device))
        .await;
    assert_eq!(owner.status, StatusCode::OK, "{}", owner.text);

    let bob = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "bob@example.com".to_owned(),
                username: "bob".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await
        .cookie("device")
        .unwrap();
    for mark in [bob.as_str(), "99999999999.forged"] {
        app.send(right_login("alice@example.com", "203.0.113.2").cookie("device", mark))
            .await
            .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    }
}

/// Buckets are keyed by addresses and numbers; the table must not hold them.
#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn stored_keys_do_not_reveal_who_they_count(pool: PgPool) {
    let app = TestApp::with(
        pool.clone(),
        Options {
            rates: Some(all(generous())),
            rate_limit_store: RateLimitStore::Postgres,
            ..Options::default()
        },
    );
    app.send(login("alice@example.com", "198.51.100.1")).await;

    let keys: Vec<String> = sqlx::query_scalar("select key from rate_limits")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!keys.is_empty());
    for key in keys {
        assert!(!key.contains("alice") && !key.contains("198.51"), "{key}");
        assert_eq!(key.len(), 32, "{key}");
    }
}
