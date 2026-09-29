//! `/api/v1/auth/oauth`: signing up and in with a social account, and linking one.
//!
//! These endpoints are browser navigations, not `fetch` calls: they answer with redirects, and
//! report failures by redirecting to the sign-in page (or the security settings when linking) with
//! `?error=<code>`.
//!
//! The short-lived `oauth` cookie binds a flow to the browser that started it. The callback always
//! clears it, then sets the session cookie, or the `mfa` cookie when a second step is due. Starting
//! and finishing a flow count as `Action::Ceremony`.

use application::{
    Adapters,
    oauth::{
        CallbackParams, OAuthOutcome,
        dto::{IdentityDto, OAuthStartQuery},
    },
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get},
};
use axum_extra::extract::CookieJar;
use domain::secret::Secret;
use serde::Deserialize;
use url::form_urlencoded;

use crate::{
    extract::{Client, CurrentUser, Form, OptionalUser, Path, Proto, Query},
    problem::ApiError,
    rate_limit::Action,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/auth/oauth/{provider}", get(start::<A>))
        .route("/auth/oauth/{provider}/link", get(start_link::<A>))
        .route(
            "/auth/oauth/{provider}/callback",
            get(callback::<A>).post(form_post_callback::<A>),
        )
        .route("/me/linked-accounts", get(linked_accounts::<A>))
        .route("/me/linked-accounts/{provider}", delete(unlink::<A>))
}

#[derive(Debug, Default, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    #[serde(default)]
    state: String,
    error: Option<String>,
}

async fn start<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Path(provider): Path<String>,
    Query(query): Query<OAuthStartQuery>,
) -> Response {
    begin(
        &state,
        &client,
        jar,
        &provider,
        None,
        query.redirect_to.as_deref(),
    )
    .await
}

async fn start_link<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    user: OptionalUser,
    jar: CookieJar,
    Path(provider): Path<String>,
) -> Response {
    match user.0 {
        Some(user) => begin(&state, &client, jar, &provider, Some(&user), None).await,
        None => failure_redirect(Flow::SignIn, "unauthenticated"),
    }
}

async fn begin<A: Adapters>(
    state: &AppState<A>,
    client: &Client,
    jar: CookieJar,
    provider: &str,
    linking: Option<&CurrentUser>,
    redirect_to: Option<&str>,
) -> Response {
    let flow = Flow::of(linking.is_some());
    if let Err(err) = state.limits.check_ip(Action::Ceremony, client.ip()).await {
        return failure_redirect(flow, err.code());
    }
    match state
        .services
        .oauth
        .start(provider, linking.map(CurrentUser::actor), redirect_to)
        .await
    {
        Ok(started) => {
            let jar = state.oauth_cookie.set(jar, &started.state);
            (jar, Redirect::to(&started.url)).into_response()
        }
        Err(err) => failure_redirect(flow, ApiError::from(err).code()),
    }
}

async fn callback<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    user: OptionalUser,
    jar: CookieJar,
    Path(provider): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let flow = Flow::of(user.0.is_some());
    if let Err(err) = state.limits.check_ip(Action::Ceremony, client.ip()).await {
        return failure_redirect(flow, err.code());
    }
    let cookie_state = state.oauth_cookie.token(&jar);
    let previous = state.cookie.token(&jar);
    let jar = state.oauth_cookie.clear(jar);

    let result = state
        .services
        .oauth
        .callback(
            CallbackParams {
                provider: &provider,
                code: query.code.map(Secret::new),
                state: &query.state,
                cookie_state: cookie_state.as_ref(),
                error: query.error.as_deref(),
            },
            user.0.as_ref().map(CurrentUser::actor),
            client.0,
            previous.as_ref(),
        )
        .await;

    match result {
        Ok(done) => match done.outcome {
            OAuthOutcome::SignedIn(signed_in) => {
                let jar = state.cookie.set(jar, &signed_in.token);
                (jar, Redirect::to(&done.redirect_to)).into_response()
            }
            OAuthOutcome::MfaRequired(required) => {
                let jar = state.mfa_cookie.set(jar, &required.token);
                let target = format!(
                    "/login/mfa?{}",
                    form_urlencoded::Serializer::new(String::new())
                        .append_pair("redirectTo", &done.redirect_to)
                        .finish()
                );
                (jar, Redirect::to(&target)).into_response()
            }
            OAuthOutcome::Linked => {
                let target = with_query_pair(&done.redirect_to, "linked", &provider);
                (jar, Redirect::to(&target)).into_response()
            }
        },
        Err(err) => (jar, failure_redirect(flow, ApiError::from(err).code())).into_response(),
    }
}

async fn form_post_callback<A: Adapters>(
    State(state): State<AppState<A>>,
    Path(provider): Path<String>,
    Form(form): Form<CallbackQuery>,
) -> Response {
    // Only a configured provider's id goes into the redirect.
    if !state
        .services
        .oauth
        .providers()
        .iter()
        .any(|configured| configured.id == provider)
    {
        return failure_redirect(Flow::SignIn, "not_found");
    }
    let mut query = form_urlencoded::Serializer::new(String::new());
    query.append_pair("state", &form.state);
    if let Some(code) = &form.code {
        query.append_pair("code", code);
    }
    if let Some(error) = &form.error {
        query.append_pair("error", error);
    }
    let target = format!("/api/v1/auth/oauth/{provider}/callback?{}", query.finish());
    Redirect::to(&target).into_response()
}

fn with_query_pair(target: &str, key: &str, value: &str) -> String {
    let (path, fragment) = target
        .split_once('#')
        .map_or((target, None), |(path, fragment)| (path, Some(fragment)));
    let pair = form_urlencoded::Serializer::new(String::new())
        .append_pair(key, value)
        .finish();
    let separator = match path.split_once('?') {
        Some((_, "")) => "",
        Some(_) => "&",
        None => "?",
    };
    let mut out = format!("{path}{separator}{pair}");
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum Flow {
    SignIn,
    Linking,
}

impl Flow {
    fn of(linking: bool) -> Self {
        if linking { Self::Linking } else { Self::SignIn }
    }
}

fn failure_redirect(flow: Flow, code: &str) -> Response {
    let page = match flow {
        Flow::Linking => "/settings/security",
        Flow::SignIn => "/login",
    };
    Redirect::to(&format!("{page}?error={code}")).into_response()
}

async fn linked_accounts<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<Vec<IdentityDto>>, ApiError> {
    Ok(Proto(state.services.oauth.identities(user.actor()).await?))
}

async fn unlink<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(provider): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.services.oauth.unlink(user.actor(), &provider).await?;
    Ok(StatusCode::NO_CONTENT)
}
