# auth

FastTrackStudio's identity: the library every app signs in through, and
the server that runs it at **auth.fasttrackstudio.app**. One account
across Task, Studio, Session, Signal, Keyflow, Ignition, Drive, Season.

It stands on its own: no dependency on architect or any app. Apps depend
on its crates; the server deploys from this repository (FastTrackStudios/
infra, host `architect-vps`).

## Crates

| Crate | What it is |
|---|---|
| `auth-proto` | The wire: accounts, sessions, orgs, devices; the three services (`AuthService`, `OrganizationService`, `DeviceService`) with their vox clients and HTTP clients (`http-client`) and routes (`http`). Builds for wasm. |
| `auth-macros` | `#[auth_macros::entity]` (a table row and its sea-orm entity), `#[auth_macros::service]` (a service's vox and HTTP faces), `#[derive(auth_macros::Config)]` (configuration from the environment). |
| `auth` | The engine: password, OAuth, passkeys, 2FA, magic links, API keys, organizations, teams, an OIDC provider; `vox_host` to serve its services over vox. |
| `auth-db` | Postgres/SQLite storage and migrations (sea-orm). |
| `auth-client` | What an app keeps: its session, its device id, the OIDC client side. |
| `architect-auth` | The engine and its parts under one name (`architect_auth::…`), as apps import it. |
| `auth-ui` | The server-rendered sign-in, account, organization and invitation pages. |
| `apps/auth-server` | The server: `/vox`, `POST /auth/…`, the OIDC provider, the pages. |
| `apps/mock-oauth` | A stand-in GitHub/Google/TONE3000 for local development. |

The wire is a published contract: `crates/auth-proto/tests/wire.rs` pins
every vox method id and HTTP path that shipped apps call.

## Develop

```sh
just test         # every crate's tests
just lint         # clippy, the house rules (pedantic, nursery, no panics)
just dev          # a throwaway server on :8080, seeded; `just mock-oauth` beside it
just build        # the server's package, as infra deploys it
```

The toolchain is the fleet's (`rust-toolchain.toml`, `nix develop`).

## Configuration

All from the environment (`apps/auth-server/src/config.rs`): `AUTH_SECRET`
(≥32 bytes, or `AUTH_SECRET_FILE`) and `AUTH_DATABASE_URL` (or its
`_FILE`) are required; `AUTH_BASE_URL`, `AUTH_OIDC_ISSUER`,
`AUTH_OIDC_CLIENTS` (JSON, public clients), `AUTH_OIDC_CLIENTS_EXTRA`
(confidential ones, from a secret), `AUTH_CORS_ORIGINS`, `AUTH_SMTP_*`,
`AUTH_GITHUB_*`, `AUTH_GOOGLE_*`, `AUTH_TONE3000_CLIENT_ID`,
`AUTH_PASSKEY_RP_ID` and the rest are optional.

## History

Split out of FastTrackStudios/architect (`features/auth`,
`apps/auth-server`, `apps/mock-oauth`) at `eaa969f1`, with what tied it
to architect replaced by its own (see the commit). The deployment that
used to live in FastTrackStudios/fts-auth is infra's now.
