use std::{
    future::{Future, ready},
    net::{IpAddr, SocketAddr},
    sync::Once,
};

use application::Adapters;
use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{HeaderMap, header::USER_AGENT, request::Parts},
};
use domain::session::ClientInfo;

use crate::state::AppState;

const X_FORWARDED_FOR: &str = "x-forwarded-for";

/// The client's IP address (for rate limiting and the session list) and user agent.
///
/// The address is the TCP peer, unless the server runs behind proxies it trusts
/// (`AppState::trusted_proxy_hops`, from `TRUST_PROXY`): then [`client_ip`] reads it from
/// `X-Forwarded-For`, which a client can otherwise forge.
#[derive(Debug, Clone)]
pub struct Client(pub ClientInfo);

impl Client {
    pub fn ip(&self) -> Option<IpAddr> {
        self.0.ip
    }
}

impl<A: Adapters> FromRequestParts<AppState<A>> for Client {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<A>,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip());
        let ip = client_ip(&parts.headers, peer, state.trusted_proxy_hops);
        let user_agent = parts
            .headers
            .get(USER_AGENT)
            .and_then(|value| value.to_str().ok());
        ready(Ok(Self(ClientInfo::new(ip, user_agent))))
    }
}

/// The TCP peer, or behind `hops` trusted proxies the `X-Forwarded-For` entry that many places
/// from the end.
///
/// Each proxy appends the address it received the request from, so the last `hops` entries were
/// written by our proxies and the one they name first is the client. Everything before it was sent
/// by the client and can be forged. With fewer entries than proxies the header is not what our
/// proxies produce, so the peer is used.
///
/// Entries may carry a port (`203.0.113.7:51234`, `[2001:db8::1]:443`), as some load balancers
/// send them.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, hops: usize) -> Option<IpAddr> {
    if hops == 0 {
        if headers.contains_key(X_FORWARDED_FOR) {
            warn_untrusted_proxy();
        }
        return peer;
    }
    let entries: Vec<&str> = headers
        .get_all(X_FORWARDED_FOR)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .collect();
    entries
        .len()
        .checked_sub(hops)
        .and_then(|index| entries.get(index))
        .and_then(|entry| parse_entry(entry.trim()))
        .or(peer)
}

fn parse_entry(entry: &str) -> Option<IpAddr> {
    entry
        .parse()
        .ok()
        .or_else(|| entry.parse::<SocketAddr>().ok().map(|addr| addr.ip()))
}

/// Says once that a proxy seems to be in front while `TRUST_PROXY` is off: every client then
/// shares the proxy's rate limit buckets. Safe, but rarely what was meant.
fn warn_untrusted_proxy() {
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        tracing::warn!(
            "requests carry X-Forwarded-For but TRUST_PROXY=false: every client counts as the \
             proxy for rate limits and sessions; set TRUST_PROXY=true if a proxy you run sets it"
        );
    });
}
