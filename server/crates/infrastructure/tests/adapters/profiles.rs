//! What each provider's answers turn into, above all whether it vouches for the address: a
//! provider that does not must never create an account with it. The JSON is shaped like the
//! providers' documented responses.

use infrastructure::oauth::{
    Provider,
    id_token::Claims,
    profiles::{accepts_issuer, github_profile, openid_profile},
};
use serde_json::json;

fn claims(email_verified: bool) -> Claims {
    Claims {
        subject: "sub-1".to_owned(),
        email: Some("alice@example.com".to_owned()),
        email_verified,
        name: Some("Alice".to_owned()),
    }
}

#[test]
fn openid_providers_vouch_through_the_email_verified_claim() {
    for provider in [Provider::Google, Provider::Apple] {
        assert!(openid_profile(provider, claims(true)).email_verified);
        assert!(!openid_profile(provider, claims(false)).email_verified);
    }
    // Any Entra tenant's admin can set `email`, whatever `email_verified` says.
    let microsoft = openid_profile(Provider::Microsoft, claims(true));
    assert!(!microsoft.email_verified);
    assert_eq!(microsoft.subject, "sub-1");
    assert_eq!(microsoft.email.as_deref(), Some("alice@example.com"));
}

#[test]
fn github_vouches_only_for_a_verified_primary_address() {
    let user = json!({ "id": 583_231, "login": "octocat", "name": null, "email": null });
    let emails = json!([
        { "email": "octocat@users.noreply.github.com", "primary": false, "verified": true, "visibility": null },
        { "email": "octo@example.com", "primary": true, "verified": true, "visibility": "public" },
    ]);
    let profile = github_profile(user.clone(), emails).unwrap();
    assert_eq!(profile.subject, "583231");
    assert_eq!(profile.email.as_deref(), Some("octo@example.com"));
    assert!(profile.email_verified);
    assert_eq!(profile.name.as_deref(), Some("octocat"));

    let unverified = json!([
        { "email": "octo@example.com", "primary": true, "verified": false, "visibility": "public" },
        { "email": "other@example.com", "primary": false, "verified": true, "visibility": null },
    ]);
    let profile = github_profile(user, unverified).unwrap();
    assert!(!profile.email_verified);
    assert_eq!(profile.email, None);
}

#[test]
fn issuers_are_checked_per_provider_and_microsoft_tenant() {
    assert!(accepts_issuer(
        Provider::Google,
        None,
        "https://accounts.google.com"
    ));
    assert!(accepts_issuer(
        Provider::Google,
        None,
        "accounts.google.com"
    ));
    assert!(accepts_issuer(
        Provider::Apple,
        None,
        "https://appleid.apple.com"
    ));
    assert!(!accepts_issuer(
        Provider::Apple,
        None,
        "https://accounts.google.com"
    ));
    assert!(!accepts_issuer(
        Provider::Github,
        None,
        "https://github.com"
    ));

    let tenant_issuer =
        "https://login.microsoftonline.com/7f2a8e11-0000-4000-8000-000000000000/v2.0";
    for tenant in [None, Some("common"), Some("organizations")] {
        assert!(accepts_issuer(Provider::Microsoft, tenant, tenant_issuer));
    }
    assert!(accepts_issuer(
        Provider::Microsoft,
        Some("7f2a8e11-0000-4000-8000-000000000000"),
        tenant_issuer
    ));
    assert!(!accepts_issuer(
        Provider::Microsoft,
        Some("aaaaaaaa-0000-4000-8000-000000000000"),
        tenant_issuer
    ));
    // Personal accounts are issued by Microsoft's fixed consumer tenant.
    assert!(accepts_issuer(
        Provider::Microsoft,
        Some("consumers"),
        "https://login.microsoftonline.com/9188040d-6c67-4c5b-b112-36a304b66dad/v2.0"
    ));
    assert!(!accepts_issuer(
        Provider::Microsoft,
        Some("consumers"),
        tenant_issuer
    ));
    assert!(!accepts_issuer(
        Provider::Microsoft,
        None,
        "https://evil.example/v2.0"
    ));
}
