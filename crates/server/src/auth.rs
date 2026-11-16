use argon2::{
    Argon2, PasswordHash,
    PasswordHasher, PasswordVerifier,
    password_hash::{
        SaltString, rand_core::OsRng,
        rand_core::RngCore,
    },
};
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

use crate::app_state::AppState;
use crate::errors::ServerError;

pub async fn auth_user_id(
  state: &AppState,
  headers: &HeaderMap
) -> Result<i64, ServerError> {
  let token = bearer_token(headers)?;

  let token_hash = hash_token(&token);

  if let Some(pool) = &state.postgres {
    let id = sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM user_tokens WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ServerError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid token"))?;

    return Ok(id);
  }

  let pool = state
        .sqlite
        .as_ref()
        .ok_or_else(|| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "database pool missing"))?;

  let id = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM user_tokens WHERE token_hash = ?1 AND expires_at > datetime('now')",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| ServerError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid token"))?;

  Ok(id)
}

pub fn bearer_token(
  headers: &HeaderMap
) -> Result<String, ServerError> {
  // Multiple Authorization fields are
  // ambiguous across HTTP proxies.
  // Never accept the first header and
  // silently ignore a conflicting one.
  let authorization = headers.get_all(
    axum::http::header::AUTHORIZATION
  );
  let mut values = authorization.iter();
  let value = match (
    values.next(),
    values.next()
  ) {
    | (Some(one), None) => {
      one.to_str().unwrap_or_default()
    }
    | _ => ""
  };

  let token = value
    .strip_prefix("Bearer ")
    .unwrap_or("")
    .trim();

  if token.is_empty() {
    return Err(ServerError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "missing bearer token",
        ));
  }

  Ok(token.to_string())
}

pub fn hash_password(
  password: &str
) -> Result<String, String> {
  let salt =
    SaltString::generate(&mut OsRng);

  let hash = Argon2::default()
    .hash_password(
      password.as_bytes(),
      &salt
    )
    .map_err(|e| {
      format!(
        "password hash error: {e}"
      )
    })?
    .to_string();

  Ok(hash)
}

pub fn verify_password(
  hash: &str,
  password: &str
) -> Result<(), String> {
  let parsed = PasswordHash::new(hash)
    .map_err(|e| {
      format!(
        "password hash parse error: \
         {e}"
      )
    })?;

  Argon2::default()
    .verify_password(
      password.as_bytes(),
      &parsed
    )
    .map_err(|e| {
      format!(
        "password verify error: {e}"
      )
    })
}

/// A deliberately unassigned account
/// hash used only to equalize
/// missing-account login work.
/// Keep Argon2 parameters aligned with
/// `hash_password`; see regression
/// tests. It never grants a session.
const DUMMY_LOGIN_HASH: &str =
  "$argon2id$v=19$m=19456,t=2,p=1$JCsB0otYwZVeTr5DLdibKQ$N/VWbnu6WU4VVpUEplmgMOPMZc0DF8AZyJsgWZAu1tw";

/// Verify exactly one Argon2 password
/// hash for both existing and missing
/// usernames. An unknown account never
/// becomes valid, even if its supplied
/// password happens to match the dummy
/// hash.
pub fn verified_login_hash<'a>(
  found_hash: Option<&'a str>,
  password: &str
) -> Option<&'a str> {
  let candidate = found_hash
    .unwrap_or(DUMMY_LOGIN_HASH);
  let matches = verify_password(
    candidate, password
  )
  .is_ok();
  if matches {
    found_hash
  } else {
    None
  }
}

pub fn generate_token() -> String {
  let mut bytes = [0u8; 32];

  OsRng.fill_bytes(&mut bytes);

  hex::encode(bytes)
}

pub fn hash_token(
  token: &str
) -> String {
  let mut hasher = Sha256::new();

  hasher.update(token.as_bytes());

  hex::encode(hasher.finalize())
}

pub async fn require_admin(
  state: &AppState,
  headers: &HeaderMap
) -> Result<i64, ServerError> {
  let user_id =
    auth_user_id(state, headers)
      .await?;
  let admin = if let Some(pool) =
    &state.postgres
  {
    sqlx::query_scalar::<_, bool>("SELECT is_admin FROM users WHERE id = $1")
      .bind(user_id).fetch_optional(pool).await
      .map_err(|e| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .unwrap_or(false)
  } else {
    let pool = state.sqlite.as_ref().ok_or_else(|| {
      ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "database pool missing")
    })?;
    sqlx::query_scalar::<_, i64>("SELECT is_admin FROM users WHERE id = ?1")
      .bind(user_id).fetch_optional(pool).await
      .map_err(|e| ServerError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
      .unwrap_or(0) != 0
  };
  if !admin {
    return Err(ServerError::new(
      axum::http::StatusCode::FORBIDDEN,
      "administrator privileges \
       required"
    ));
  }
  Ok(user_id)
}

#[cfg(test)]
mod bearer_header_tests {
  use axum::http::header::AUTHORIZATION;
  use axum::http::{
    HeaderValue,
    StatusCode
  };
  use axum::response::IntoResponse;

  use super::*;

  #[test]
  fn missing_login_account_runs_dummy_argon2()
   {
    let password =
      "valid-fixture-password";
    let hash = hash_password(password)
      .expect("fixture password hash");

    assert_eq!(
      verified_login_hash(
        Some(&hash),
        password
      ),
      Some(hash.as_str())
    );
    assert!(
      verified_login_hash(
        Some(&hash),
        "bad-password"
      )
      .is_none()
    );
    assert!(
      verified_login_hash(
        None, password
      )
      .is_none()
    );
    // Even the dummy's actual preimage
    // cannot authenticate a missing
    // user.
    assert!(
      verify_password(
        DUMMY_LOGIN_HASH,
        "pulsewire-decoy-only-not-a-real-login"
      )
      .is_ok()
    );
    assert!(
      verified_login_hash(
        None,
        "pulsewire-decoy-only-not-a-real-login"
      )
      .is_none()
    );

    // Fail CI if the production Argon2
    // default cost drifts away from the
    // dummy work factor.
    let actual =
      PasswordHash::new(&hash)
        .expect("actual PHC hash");
    let dummy = PasswordHash::new(
      DUMMY_LOGIN_HASH
    )
    .expect("dummy PHC hash");
    assert_eq!(
      actual.algorithm.as_str(),
      dummy.algorithm.as_str()
    );
    assert_eq!(
      actual.version,
      dummy.version
    );
    assert_eq!(
      actual.params.to_string(),
      dummy.params.to_string()
    );
  }

  #[test]
  fn duplicate_authorization_fields_are_denied()
   {
    let token = "a".repeat(64);
    let good = HeaderValue::from_str(
      &format!("Bearer {token}")
    )
    .expect("valid header");
    let mut headers = HeaderMap::new();
    assert_eq!(
      bearer_token(&headers)
        .expect_err("missing header")
        .into_response()
        .status(),
      StatusCode::UNAUTHORIZED
    );
    headers.append(
      AUTHORIZATION,
      good.clone()
    );
    assert_eq!(
      bearer_token(&headers)
        .expect("one valid header"),
      token
    );
    headers.append(
      AUTHORIZATION,
      HeaderValue::from_static(
        "Bearer deliberately-invalid"
      )
    );
    assert_eq!(
      bearer_token(&headers)
        .expect_err("duplicate headers")
        .into_response()
        .status(),
      StatusCode::UNAUTHORIZED
    );
    headers.clear();
    headers.append(
      AUTHORIZATION,
      HeaderValue::from_static(
        "Bearer deliberately-invalid"
      )
    );
    headers.append(
      AUTHORIZATION,
      good.clone()
    );
    assert_eq!(
      bearer_token(&headers)
        .expect_err(
          "reversed duplicates"
        )
        .into_response()
        .status(),
      StatusCode::UNAUTHORIZED
    );
    headers.clear();
    headers.append(
      AUTHORIZATION,
      good.clone()
    );
    headers.append(AUTHORIZATION, good);
    assert_eq!(
      bearer_token(&headers)
        .expect_err(
          "identical duplicates"
        )
        .into_response()
        .status(),
      StatusCode::UNAUTHORIZED
    );
  }
}
