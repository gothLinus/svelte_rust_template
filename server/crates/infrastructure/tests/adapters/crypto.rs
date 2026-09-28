use std::collections::HashSet;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    clock::Clock,
    secret::{MAX_SECRET_BYTES, Secret},
    security::{HashError, PasswordHasher, TokenGenerator},
    user::PasswordHash,
};
use infrastructure::{
    clock::SystemClock,
    crypto::{Argon2Hasher, Argon2Params, RandomTokens, TOKEN_BYTES},
};

fn fast() -> Argon2Params {
    Argon2Params {
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
        max_concurrent: 2,
        max_queued: 64,
    }
}

#[tokio::test]
async fn hashes_are_argon2id_phc_strings_and_verify() {
    let hasher = Argon2Hasher::new(fast()).unwrap();
    let password = Secret::new("correct horse");

    let hash = hasher.hash(&password).await.unwrap();

    assert!(hash.as_str().starts_with("$argon2id$v=19$m=64,t=1,p=1$"));
    assert!(hasher.verify(&password, Some(&hash)).await.unwrap());
    assert!(
        !hasher
            .verify(&Secret::new("wrong horse"), Some(&hash))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn hashes_are_salted() {
    let hasher = Argon2Hasher::new(fast()).unwrap();
    let password = Secret::new("correct horse");
    let first = hasher.hash(&password).await.unwrap();
    let second = hasher.hash(&password).await.unwrap();
    assert_ne!(first, second);
}

#[tokio::test]
async fn verifying_without_a_hash_always_fails() {
    let hasher = Argon2Hasher::new(fast()).unwrap();
    // Even the decoy password itself does not match "no account".
    for password in ["", "decoy password that equalises sign-in timing", "x"] {
        assert!(!hasher.verify(&Secret::new(password), None).await.unwrap());
    }
}

#[tokio::test]
async fn oversized_and_malformed_inputs_do_not_match() {
    let hasher = Argon2Hasher::new(fast()).unwrap();
    let huge = Secret::new("x".repeat(MAX_SECRET_BYTES + 1));
    let hash = hasher.hash(&Secret::new("x")).await.unwrap();

    assert!(!hasher.verify(&huge, Some(&hash)).await.unwrap());
    assert!(
        !hasher
            .verify(
                &Secret::new("x"),
                Some(&domain::user::PasswordHash::new("not a phc string"))
            )
            .await
            .unwrap()
    );
}

#[test]
fn params_are_validated() {
    assert!(Argon2Params::default().validate().is_ok());
    assert!(
        Argon2Params {
            memory_kib: 1,
            ..fast()
        }
        .validate()
        .is_err()
    );
    assert!(
        Argon2Params {
            max_concurrent: 0,
            ..fast()
        }
        .validate()
        .is_err()
    );
    assert!(
        Argon2Hasher::new(Argon2Params {
            iterations: 0,
            ..fast()
        })
        .is_err()
    );
}

#[test]
fn tokens_are_random_url_safe_and_256_bits() {
    let tokens = RandomTokens;
    let generated: HashSet<String> = (0..100)
        .map(|_| tokens.generate().unwrap().expose().to_owned())
        .collect();

    assert_eq!(generated.len(), 100);
    for token in &generated {
        assert_eq!(token.len(), 43);
        assert_eq!(URL_SAFE_NO_PAD.decode(token).unwrap().len(), TOKEN_BYTES);
    }
}

#[test]
fn digests_are_sha256_of_the_token() {
    let tokens = RandomTokens;
    let digest = tokens.digest(&Secret::new("abc"));
    // SHA-256("abc")
    assert_eq!(digest.as_bytes()[..4], [0xba, 0x78, 0x16, 0xbf]);
    assert_eq!(digest, tokens.digest(&Secret::new("abc")));
    assert_ne!(digest, tokens.digest(&Secret::new("abd")));
}

#[test]
fn the_system_clock_has_microsecond_precision() {
    let now = SystemClock.now();
    assert_eq!(now.nanosecond() % 1_000, 0);
    assert!((time::OffsetDateTime::now_utc() - now).abs() < time::Duration::seconds(5));
}

#[tokio::test]
async fn hashes_with_other_parameters_need_a_rehash() {
    let current = Argon2Hasher::new(fast()).unwrap();
    let password = Secret::new("correct horse battery");
    let fresh = current.hash(&password).await.unwrap();
    assert!(!current.needs_rehash(&fresh));

    let cheaper = Argon2Hasher::new(Argon2Params {
        memory_kib: 32,
        ..fast()
    })
    .unwrap();
    let old = cheaper.hash(&password).await.unwrap();
    assert!(current.verify(&password, Some(&old)).await.unwrap());
    assert!(current.needs_rehash(&old));
    assert!(!current.needs_rehash(&PasswordHash::new("not a phc string")));
}

#[tokio::test]
async fn a_full_queue_refuses_at_once() {
    // Real cost, so the first hash is still running when the second asks: with the cheap test
    // parameters it can finish on its thread before `join!` polls the second.
    let hasher = Argon2Hasher::new(Argon2Params {
        max_concurrent: 1,
        max_queued: 0,
        ..Argon2Params::default()
    })
    .unwrap();
    let password = Secret::new("correct horse battery staple");

    // `join!` polls in order: the first takes the only place, so the second finds none.
    let (first, second) = tokio::join!(hasher.hash(&password), hasher.hash(&password));
    assert!(first.is_ok());
    assert!(matches!(second, Err(HashError::Busy)));

    assert!(hasher.hash(&password).await.is_ok());
}
