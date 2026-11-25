# auth: FastTrackStudio's identity library and its server.

default:
    @just --list

check:
    cargo check --workspace --all-targets --all-features

lint:
    cargo clippy --workspace --all-targets --all-features

test:
    cargo test --workspace --all-features

# The server's package, as infra deploys it.
build:
    nix build .#auth-server

# The auth pages' sheet: generated, and committed. Rebuild after using a
# new class in auth-ui (its own components included); a class no source
# uses gets no rule, and Tailwind does not warn.
auth-ui-css:
    cd crates/auth-ui && tailwindcss -i tailwind.css -o assets/utilities.css --minify

auth-ui-css-check: auth-ui-css
    #!/usr/bin/env bash
    set -euo pipefail
    if ! git diff --quiet -- crates/auth-ui/assets/utilities.css; then
        echo "auth-ui's utilities.css is out of date: run 'just auth-ui-css' and commit it" >&2
        exit 1
    fi

# A throwaway server on http://localhost:8080 (in-memory database, the
# development cast seeded, everyone's password `development-password`),
# social sign-in pointed at `just mock-oauth`.
dev:
    AUTH_BIND_ADDR=127.0.0.1:8080 \
    AUTH_BASE_URL=http://localhost:8080 \
    AUTH_DATABASE_URL='sqlite::memory:' \
    AUTH_SECRET=development-secret-development-secret \
    AUTH_DEV_SEED=1 \
    AUTH_SOCIAL_MOCK_URL=http://localhost:4040 \
    AUTH_GITHUB_CLIENT_ID=mock-github AUTH_GITHUB_CLIENT_SECRET=mock-github-secret \
    AUTH_GOOGLE_CLIENT_ID=mock-google AUTH_GOOGLE_CLIENT_SECRET=mock-google-secret \
    AUTH_TONE3000_CLIENT_ID=mock-tone3000 \
    cargo run -p auth-server

# A stand-in for GitHub, Google and TONE3000 on http://localhost:4040. It
# verifies nothing, and says so on every page.
mock-oauth:
    cargo run -p mock-oauth
