//! Organizations, minus the plumbing: the requests an app sends the
//! issuer to make and change its person's organizations, and how it
//! reads the answers.
//!
//! Like [`crate::oidc`], no HTTP: an app sends the request the way it
//! already sends things (a browser's `fetch`, a native client) and hands
//! the status and body back here. The issuer's HTTP tests send exactly
//! these requests (`apps/auth-server/tests/org_create.rs`), so a change
//! on either side fails there first.
//!
//! The token is the person's: a session token, or the access token of
//! an app signed in by redirect as a first-party client (Studio through
//! Drive's `drive`), sent as `Authorization: Bearer`. Creating takes
//! either (`r[auth.jwt.first-party-access]`).
//!
//! ```ignore
//! let body = orgs::create_request("Adult Jam")?;
//! let (status, text) = http
//!     .post(format!("{issuer}{}", orgs::CREATE_PATH))
//!     .bearer(token)
//!     .json_body(body)
//!     .send()
//!     .await?;
//! let org = orgs::created_from(status, &text)?; // org.role == "owner"
//! ```

/// Where an organization is created, relative to the issuer. `POST`,
/// `content-type: application/json`, the body [`create_request`] makes.
pub const CREATE_PATH: &str = "/auth/organization/create";

/// An organization of the person's, as the issuer answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    /// The issuer's id (what its other organization calls name it by).
    pub id: String,
    pub name: String,
    /// What apps key on: lower-case letters, digits and `-`.
    pub slug: String,
    /// The person's role in it: `owner` for one they just made.
    pub role: String,
}

/// What went wrong making or reading an organization request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrgError {
    /// The name has no letter or digit to make a slug of.
    #[error("an organization's name needs a letter or a digit")]
    NoName,
    /// The token is missing, expired, signed out, or not one the issuer
    /// takes here (a third-party app's): sign in again.
    #[error("sign in to do that")]
    SignedOut,
    /// Another organization already has this name's slug.
    #[error("an organization with that name already exists")]
    Taken,
    /// The issuer refused for another reason; its code and message.
    #[error("the issuer refused ({code}): {message}")]
    Refused { code: String, message: String },
    /// The answer was not what the issuer sends.
    #[error("the issuer's answer could not be read: {0}")]
    Malformed(String),
}

/// The slug a name makes: lower-case ASCII letters and digits, every
/// run of anything else one `-`, none at either end. `None` when nothing
/// is left. What the issuer accepts, so an app shows it before asking.
#[must_use]
pub fn slug_for(name: &str) -> Option<String> {
    let mut slug = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    (!slug.is_empty()).then_some(slug)
}

/// The JSON body that creates an organization called `name` (its slug
/// from [`slug_for`]); the caller becomes its owner.
///
/// # Errors
///
/// [`OrgError::NoName`] when the name makes no slug.
pub fn create_request(name: &str) -> Result<String, OrgError> {
    let name = name.trim();
    let slug = slug_for(name).ok_or(OrgError::NoName)?;
    Ok(serde_json::json!({
        "organization": {
            "name": name,
            "slug": slug,
            "logo": null,
            "metadata_json": null,
        }
    })
    .to_string())
}

/// Read the issuer's answer to [`create_request`]: the new organization,
/// with the caller's role in it.
///
/// # Errors
///
/// [`OrgError::SignedOut`], [`OrgError::Taken`], [`OrgError::Refused`]
/// as the issuer says; [`OrgError::Malformed`] for anything else.
pub fn created_from(status: u16, body: &str) -> Result<Organization, OrgError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| OrgError::Malformed(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(refusal(status, &json));
    }
    let text = |v: &serde_json::Value, field: &'static str| {
        v.get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| OrgError::Malformed(format!("no {field}")))
    };
    let org = json
        .get("organization")
        .ok_or_else(|| OrgError::Malformed("no organization".into()))?;
    let membership = json
        .get("membership")
        .ok_or_else(|| OrgError::Malformed("no membership".into()))?;
    Ok(Organization {
        id: text(org, "id")?,
        name: text(org, "name")?,
        slug: text(org, "slug")?,
        role: text(membership, "role")?,
    })
}

/// The issuer's error envelope (`{code, message, error}`) as an
/// [`OrgError`].
fn refusal(status: u16, json: &serde_json::Value) -> OrgError {
    let field = |k: &str| {
        json.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let (code, message) = (field("code"), field("message"));
    match code.as_str() {
        "invalid_credentials" | "session_expired" => OrgError::SignedOut,
        _ if status == 401 => OrgError::SignedOut,
        "invalid_input" if message.contains("already exists") => OrgError::Taken,
        _ => OrgError::Refused { code, message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_makes_the_slug_the_issuer_takes() {
        assert_eq!(slug_for("Adult Jam!").as_deref(), Some("adult-jam"));
        assert_eq!(
            slug_for("  Tom Brooks   Music ").as_deref(),
            Some("tom-brooks-music")
        );
        assert_eq!(slug_for("CBU").as_deref(), Some("cbu"));
        assert_eq!(slug_for("Église St-Jean").as_deref(), Some("glise-st-jean"));
        assert_eq!(slug_for("2026 Band").as_deref(), Some("2026-band"));
        assert_eq!(slug_for("!!!"), None);
        assert_eq!(slug_for(""), None);
    }

    #[test]
    fn a_nameless_request_is_refused_before_it_is_sent() {
        assert_eq!(create_request("  ?? "), Err(OrgError::NoName));
    }

    #[test]
    fn a_reply_that_is_not_the_issuers_is_malformed() {
        assert!(matches!(
            created_from(502, "<html>bad gateway</html>"),
            Err(OrgError::Malformed(_))
        ));
        assert!(matches!(
            created_from(200, r#"{"organization":{"id":"x"}}"#),
            Err(OrgError::Malformed(_))
        ));
    }
}
