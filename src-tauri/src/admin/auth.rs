use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::http::HeaderMap;
use serde::Deserialize;

pub const ADMIN_SESSION_COOKIE: &str = "coding_tools_admin_session";
const SESSION_TTL_SECS: u64 = 12 * 60 * 60;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Default)]
pub struct AdminSessionStore {
    sessions: Mutex<HashMap<String, u64>>,
}

impl AdminSessionStore {
    pub fn create(&self) -> String {
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let expires_at = now_secs().saturating_add(SESSION_TTL_SECS);
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(token.clone(), expires_at);
        token
    }

    pub fn is_authenticated(&self, headers: &HeaderMap) -> bool {
        let Some(token) = token_from_headers(headers) else {
            return false;
        };
        let now = now_secs();
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sessions.retain(|_, expires_at| *expires_at > now);
        sessions.get(token).is_some_and(|expires_at| *expires_at > now)
    }

    pub fn revoke_from_headers(&self, headers: &HeaderMap) {
        if let Some(token) = token_from_headers(headers) {
            self.sessions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(token);
        }
    }
}

pub fn validate_credentials(credentials: &AdminCredentials) -> Result<(), String> {
    let username = credentials.username.trim();
    if username.len() < 3 || username.len() > 64 {
        return Err("管理员用户名长度需为 3-64 个字符".into());
    }
    if credentials.password.chars().count() < 8 {
        return Err("管理员密码至少需要 8 个字符".into());
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("生成管理员密码哈希失败: {error}"))
}

pub fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn session_cookie(token: &str) -> String {
    format!(
        "{ADMIN_SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_TTL_SECS}"
    )
}

pub fn clear_session_cookie() -> String {
    format!(
        "{ADMIN_SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
    )
}

fn token_from_headers(headers: &HeaderMap) -> Option<&str> {
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == ADMIN_SESSION_COOKIE && !value.is_empty()).then_some(value)
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, HeaderValue};

    #[test]
    fn password_hash_round_trip() {
        let hash = hash_password("correct horse battery staple").expect("hash");
        assert_ne!(hash, "correct horse battery staple");
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong-password", &hash));
    }

    #[test]
    fn session_cookie_authenticates_and_can_be_revoked() {
        let store = AdminSessionStore::default();
        let token = store.create();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{ADMIN_SESSION_COOKIE}={token}"))
                .expect("cookie header"),
        );
        assert!(store.is_authenticated(&headers));
        store.revoke_from_headers(&headers);
        assert!(!store.is_authenticated(&headers));
    }
}
