//! Creating an organization from an app signed in by redirect, over the
//! real HTTP surface: the person signs in through `/oauth2/authorize` as
//! a first-party client (Drive's `drive`, which Studio signs in through),
//! holds only the access token `/oauth2/token` gave, and creates an org
//! with the requests `auth_client::orgs` builds, read back the way it
//! reads them. What an app sends is checked against what the issuer
//! answers, here, so neither can drift from the other.

// An integration-test crate: an `unwrap()` here IS the assertion (see
// `http_surface.rs`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use architect_auth::OidcClientConfig;
use architect_auth::db::{AuthSeaOrmStorage, Migrator};
use auth_client::oidc::{self, Pkce};
use auth_client::orgs::{self, OrgError};
use auth_server::{ServerConfig, server};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use tower::ServiceExt;

const REDIRECT: &str = "https://fasttrackstudio.app/studio/auth/callback";

fn client(id: &str, first_party: bool) -> OidcClientConfig {
    OidcClientConfig {
        client_id: id.into(),
        client_secret: None,
        name: id.into(),
        redirect_uris: vec![REDIRECT.into()],
        scopes: vec!["openid".into(), "email".into(), "profile".into()],
        public_client: true,
        skip_consent: true,
        first_party,
        rotate_refresh_tokens: false,
        disabled: false,
        issue_sessions: false,
    }
}

async fn app() -> axum::Router {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    Migrator::up(&db, None).await.expect("migrate");
    let config = ServerConfig {
        bind_addr: "127.0.0.1:0".into(),
        base_url: "https://auth.fasttrackstudio.app".into(),
        session_ttl_seconds: 3600,
        oidc_clients: vec![client("drive", true), client("elsewhere", false)],
        ..ServerConfig::local()
    };
    let auth = server::build_engine(&config, AuthSeaOrmStorage::new(db)).expect("build engine");
    server::app_router(&config, auth).expect("router builds")
}

async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, String, String) {
    let response = app.clone().oneshot(request).await.expect("request");
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        String::from_utf8_lossy(&bytes).into_owned(),
        location,
    )
}

/// A new account's session token.
async fn signed_up(app: &axum::Router, email: &str) -> String {
    let (status, body, _) = send(
        app,
        Request::post("/auth/sign-up/email")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(format!(
                r#"{{"input":{{"email":"{email}","password":"correct horse battery staple"}}}}"#
            )))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "sign up {email}: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    json["token"].as_str().expect("a session token").to_owned()
}

/// What an app signed in by redirect holds: `client_id`'s access token,
/// from the code flow a browser goes through (the session rides the
/// cookie, as it would in the browser).
async fn access_token(app: &axum::Router, session: &str, client_id: &str) -> String {
    let pkce = Pkce::from_entropy([7; 32], [9; 16]);
    let url = oidc::authorize_url("", client_id, REDIRECT, &pkce, oidc::DEFAULT_SCOPE);
    let (status, body, location) = send(
        app,
        Request::get(url)
            .header(header::COOKIE, format!("architect-auth.session={session}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "authorize: {body}");
    let code = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("code="))
        .unwrap_or_else(|| panic!("no code in {location}"));
    let (status, body, _) = send(
        app,
        Request::post(oidc::TOKEN_PATH)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(oidc::token_request_body(
                client_id, REDIRECT, code, &pkce,
            )))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "token: {body}");
    oidc::access_token_from(&body).expect("an access token")
}

/// Create `name` with `token`, as an app does: `auth_client::orgs`'s
/// request, its reading of the answer.
async fn create(
    app: &axum::Router,
    token: &str,
    name: &str,
) -> Result<orgs::Organization, OrgError> {
    let request = orgs::create_request(name)?;
    let (status, body, _) = send(
        app,
        Request::post(orgs::CREATE_PATH)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(request))
            .unwrap(),
    )
    .await;
    orgs::created_from(status.as_u16(), &body)
}

#[tokio::test]
async fn a_person_signed_in_by_redirect_creates_an_organization_and_owns_it() {
    let app = app().await;
    let session = signed_up(&app, "ada@example.com").await;
    let token = access_token(&app, &session, "drive").await;

    let made = create(&app, &token, "Adult Jam!").await.expect("created");
    assert_eq!(made.slug, "adult-jam");
    assert_eq!(made.name, "Adult Jam!");
    assert_eq!(made.role, "owner");
    assert!(!made.id.is_empty(), "the issuer's id comes back");

    // The org list (what an org's node relays to the app) has it, with
    // the creator as owner.
    let (status, body, _) = send(
        &app,
        Request::post("/auth/organization/list")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "list: {body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let mine: Vec<(String, String)> = listed
        .as_array()
        .expect("a list")
        .iter()
        .map(|b| {
            (
                b["organization"]["slug"].as_str().unwrap().to_owned(),
                b["membership"]["role"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(mine, [("adult-jam".to_owned(), "owner".to_owned())]);
}

#[tokio::test]
async fn a_name_already_taken_is_said_so() {
    let app = app().await;
    let ada = signed_up(&app, "ada@example.com").await;
    let ada = access_token(&app, &ada, "drive").await;
    let bo = signed_up(&app, "bo@example.com").await;
    let bo = access_token(&app, &bo, "drive").await;

    create(&app, &ada, "Days to Praise").await.expect("created");
    assert_eq!(
        create(&app, &bo, "days to praise").await,
        Err(OrgError::Taken)
    );
}

#[tokio::test]
async fn a_third_partys_token_or_none_is_asked_to_sign_in() {
    let app = app().await;
    let session = signed_up(&app, "ada@example.com").await;
    let elsewhere = access_token(&app, &session, "elsewhere").await;

    assert_eq!(
        create(&app, &elsewhere, "Band").await,
        Err(OrgError::SignedOut)
    );
    assert_eq!(
        create(&app, "not-a-token", "Band").await,
        Err(OrgError::SignedOut)
    );
}
