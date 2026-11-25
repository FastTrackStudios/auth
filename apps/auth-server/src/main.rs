//! `auth-server` — run the `FastTrackStudio` identity server.
//!
//! Configuration is entirely environmental, so the same image serves
//! every environment. See [`auth_server::config`] for the full list;
//! the two required vars are `AUTH_SECRET` (or `AUTH_SECRET_FILE`) and
//! `AUTH_DATABASE_URL` (or `AUTH_DATABASE_URL_FILE`).

use auth_server::{ServerConfig, server};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    auth_server::host::boot("info,auth_server=debug");

    // `load`, not the derived `from_env`: only `load` reads the OIDC
    // clients (and merges the confidential extras).
    let config = ServerConfig::load()?;

    // Loudly rather than quietly issuing tokens no one can trust: the
    // issuer is the value relying parties pin.
    if config.base_url.starts_with("http://localhost") {
        tracing::warn!(
            "AUTH_BASE_URL is still the localhost default — OIDC discovery will \
             advertise unreachable endpoints"
        );
    }
    if config.oidc_clients.is_empty() {
        tracing::warn!(
            "no OIDC clients configured (AUTH_OIDC_CLIENTS) — only the vox and \
             session-JSON surfaces will be usable"
        );
    }
    tracing::info!(
        base_url = %config.base_url,
        issuer = %config.issuer(),
        oidc_clients = config.oidc_clients.len(),
        "starting auth server"
    );

    let built = server::build(&config).await?;
    server::serve(built).await
}
