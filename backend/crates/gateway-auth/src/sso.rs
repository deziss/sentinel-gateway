//! OAuth2 / OIDC providers (Keycloak, Okta, Google, GitHub, Microsoft, generic OIDC).
//!
//! All providers implement the same `OAuth2Provider` trait which handles:
//!   1. Building the authorization URL (state + PKCE + nonce)
//!   2. Exchanging the authorization code for an access token
//!   3. Fetching user info (normalized across providers)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AuthError;

/// Normalized user profile returned by any provider after code exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoUserInfo {
    pub provider: String,
    pub provider_user_id: String,
    pub email: String,
    /// Whether the provider has verified this email
    #[serde(default)]
    pub email_verified: bool,
    pub name: Option<String>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub groups: Vec<String>,
    /// Raw provider profile for audit / debugging.
    pub raw_profile: Value,
}

/// Token-exchange result from the provider's `/token` endpoint.
#[derive(Debug, Clone)]
pub struct TokenExchangeResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in_secs: Option<i64>,
    pub user_info: SsoUserInfo,
}

/// Provider config — passed to each provider constructor.
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub kind: ProviderKind,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    /// OIDC discovery base URL (used by Keycloak/Okta/Microsoft/Generic).
    pub issuer_url: Option<String>,
    /// Explicit endpoint overrides.
    pub authorize_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Keycloak,
    Okta,
    Google,
    Github,
    Microsoft,
    OidcGeneric,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Keycloak => "keycloak",
            ProviderKind::Okta => "okta",
            ProviderKind::Google => "google",
            ProviderKind::Github => "github",
            ProviderKind::Microsoft => "microsoft",
            ProviderKind::OidcGeneric => "oidc_generic",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "keycloak" => Some(ProviderKind::Keycloak),
            "okta" => Some(ProviderKind::Okta),
            "google" => Some(ProviderKind::Google),
            "github" => Some(ProviderKind::Github),
            "microsoft" => Some(ProviderKind::Microsoft),
            "oidc_generic" => Some(ProviderKind::OidcGeneric),
            _ => None,
        }
    }
}

/// Trait every OAuth2 provider implements.
#[async_trait]
pub trait OAuth2Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    /// Build the provider's `/authorize` URL with state + optional PKCE + nonce.
    fn build_authorize_url(
        &self,
        state: &str,
        code_challenge: Option<&str>,
        nonce: Option<&str>,
    ) -> Result<String, AuthError>;

    /// Exchange an authorization code for tokens + user info.
    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<TokenExchangeResult, AuthError>;
}

// ── Shared helpers ──────────────────────────────────────────────────────────

async fn post_form(
    url: &str,
    params: &[(&str, &str)],
    auth_header: Option<&str>,
) -> Result<Value, AuthError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AuthError::Internal(format!("http client: {e}")))?;

    let mut req = client.post(url)
        .header("Accept", "application/json")
        .form(params);
    if let Some(h) = auth_header {
        req = req.header("Authorization", h);
    }
    let resp = req.send().await
        .map_err(|e| AuthError::Internal(format!("token exchange: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AuthError::Unauthorized(format!("token endpoint: {body}")));
    }

    resp.json::<Value>().await
        .map_err(|e| AuthError::Internal(format!("invalid token response: {e}")))
}

async fn get_json(url: &str, bearer: &str) -> Result<Value, AuthError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AuthError::Internal(format!("http client: {e}")))?;
    let resp = client.get(url)
        .bearer_auth(bearer)
        .header("Accept", "application/json")
        .header("User-Agent", "sentinel-gateway/1.0")
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("userinfo fetch: {e}")))?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AuthError::Unauthorized(format!("userinfo: {body}")));
    }
    resp.json::<Value>().await
        .map_err(|e| AuthError::Internal(format!("invalid userinfo: {e}")))
}

fn url_encode(s: &str) -> String {
    // Minimal URL encoding — avoids adding urlencoding dep just for this
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xF) as usize] as char);
            }
        }
    }
    out
}

fn build_query(scopes: &[String], extra: &[(&str, String)]) -> String {
    let mut params: Vec<String> = Vec::new();
    let scope_str = scopes.join(" ");
    params.push(format!("scope={}", url_encode(&scope_str)));
    for (k, v) in extra {
        params.push(format!("{k}={}", url_encode(v)));
    }
    params.join("&")
}

// ── OIDC-style providers (Keycloak, Okta, Google, Microsoft, Generic) ──────

/// Generic OIDC provider — handles any RFC-compliant provider via discovery.
pub struct OidcProvider {
    cfg: OAuth2Config,
}

impl OidcProvider {
    pub fn new(cfg: OAuth2Config) -> Self {
        Self { cfg }
    }

    fn authorize_endpoint(&self) -> Result<String, AuthError> {
        if let Some(u) = &self.cfg.authorize_url {
            return Ok(u.clone());
        }
        let issuer = self.cfg.issuer_url.as_ref()
            .ok_or_else(|| AuthError::Internal("no issuer_url or authorize_url".into()))?;
        Ok(format!(
            "{}/protocol/openid-connect/auth",
            issuer.trim_end_matches('/')
        ))
    }

    fn token_endpoint(&self) -> Result<String, AuthError> {
        if let Some(u) = &self.cfg.token_url {
            return Ok(u.clone());
        }
        let issuer = self.cfg.issuer_url.as_ref()
            .ok_or_else(|| AuthError::Internal("no issuer_url or token_url".into()))?;
        Ok(format!(
            "{}/protocol/openid-connect/token",
            issuer.trim_end_matches('/')
        ))
    }

    fn userinfo_endpoint(&self) -> Result<String, AuthError> {
        if let Some(u) = &self.cfg.userinfo_url {
            return Ok(u.clone());
        }
        let issuer = self.cfg.issuer_url.as_ref()
            .ok_or_else(|| AuthError::Internal("no issuer_url or userinfo_url".into()))?;
        Ok(format!(
            "{}/protocol/openid-connect/userinfo",
            issuer.trim_end_matches('/')
        ))
    }
}

#[async_trait]
impl OAuth2Provider for OidcProvider {
    fn kind(&self) -> ProviderKind {
        self.cfg.kind
    }

    fn build_authorize_url(
        &self,
        state: &str,
        code_challenge: Option<&str>,
        nonce: Option<&str>,
    ) -> Result<String, AuthError> {
        let mut extra: Vec<(&str, String)> = vec![
            ("client_id", self.cfg.client_id.clone()),
            ("redirect_uri", self.cfg.redirect_uri.clone()),
            ("response_type", "code".to_string()),
            ("state", state.to_string()),
        ];
        if let Some(cc) = code_challenge {
            extra.push(("code_challenge", cc.to_string()));
            extra.push(("code_challenge_method", "S256".to_string()));
        }
        if let Some(n) = nonce {
            extra.push(("nonce", n.to_string()));
        }
        let query = build_query(&self.cfg.scopes, &extra);
        Ok(format!("{}?{}", self.authorize_endpoint()?, query))
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<TokenExchangeResult, AuthError> {
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.cfg.redirect_uri.as_str()),
            ("client_id", self.cfg.client_id.as_str()),
            ("client_secret", self.cfg.client_secret.as_str()),
        ];
        if let Some(v) = code_verifier {
            params.push(("code_verifier", v));
        }
        let token_resp = post_form(&self.token_endpoint()?, &params, None).await?;

        let access_token = token_resp.get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::Unauthorized("no access_token".into()))?
            .to_string();

        let refresh_token = token_resp.get("refresh_token")
            .and_then(|v| v.as_str())
            .map(String::from);
        let id_token = token_resp.get("id_token")
            .and_then(|v| v.as_str())
            .map(String::from);
        let expires_in_secs = token_resp.get("expires_in").and_then(|v| v.as_i64());

        // Fetch userinfo
        let profile = get_json(&self.userinfo_endpoint()?, &access_token).await?;
        let user_info = normalize_oidc_profile(self.cfg.kind, &profile);

        Ok(TokenExchangeResult {
            access_token,
            refresh_token,
            id_token,
            expires_in_secs,
            user_info,
        })
    }
}

fn normalize_oidc_profile(kind: ProviderKind, p: &Value) -> SsoUserInfo {
    // OIDC `sub` is the canonical user ID. Email/name/picture are standard claims.
    let sub = p.get("sub").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let email = p.get("email").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let email_verified = p.get("email_verified").and_then(|v| v.as_bool()).unwrap_or(false);
    let name = p.get("name").and_then(|v| v.as_str()).map(String::from);
    let username = p.get("preferred_username").and_then(|v| v.as_str()).map(String::from);
    let avatar_url = p.get("picture").and_then(|v| v.as_str()).map(String::from);

    // Groups: Keycloak uses "groups"; Okta uses "groups"; Google uses nothing standard.
    let groups = p.get("groups")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|g| g.as_str().map(String::from)).collect())
        .unwrap_or_default();

    SsoUserInfo {
        provider: kind.as_str().to_string(),
        provider_user_id: sub,
        email,
        email_verified,
        name,
        username,
        avatar_url,
        groups,
        raw_profile: p.clone(),
    }
}

// ── GitHub (non-OIDC; custom endpoints) ─────────────────────────────────────

pub struct GithubProvider {
    cfg: OAuth2Config,
}

impl GithubProvider {
    pub fn new(mut cfg: OAuth2Config) -> Self {
        cfg.kind = ProviderKind::Github;
        if cfg.scopes.is_empty() {
            cfg.scopes = vec!["read:user".into(), "user:email".into()];
        }
        Self { cfg }
    }
}

#[async_trait]
impl OAuth2Provider for GithubProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Github
    }

    fn build_authorize_url(
        &self,
        state: &str,
        _code_challenge: Option<&str>,
        _nonce: Option<&str>,
    ) -> Result<String, AuthError> {
        // GitHub OAuth Apps don't support PKCE (GitHub Apps do — we support the common case)
        let query = build_query(&self.cfg.scopes, &[
            ("client_id", self.cfg.client_id.clone()),
            ("redirect_uri", self.cfg.redirect_uri.clone()),
            ("state", state.to_string()),
            ("allow_signup", "true".to_string()),
        ]);
        Ok(format!("https://github.com/login/oauth/authorize?{query}"))
    }

    async fn exchange_code(
        &self,
        code: &str,
        _code_verifier: Option<&str>,
    ) -> Result<TokenExchangeResult, AuthError> {
        let params = vec![
            ("client_id", self.cfg.client_id.as_str()),
            ("client_secret", self.cfg.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", self.cfg.redirect_uri.as_str()),
        ];
        let token_resp = post_form(
            "https://github.com/login/oauth/access_token",
            &params,
            None,
        ).await?;

        let access_token = token_resp.get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::Unauthorized("no access_token".into()))?
            .to_string();

        // GitHub user profile
        let profile = get_json("https://api.github.com/user", &access_token).await?;

        // Fetch primary verified email (can be private by default)
        let email = match get_json("https://api.github.com/user/emails", &access_token).await {
            Ok(Value::Array(emails)) => emails.iter()
                .find(|e| e.get("primary").and_then(|v| v.as_bool()).unwrap_or(false)
                       && e.get("verified").and_then(|v| v.as_bool()).unwrap_or(false))
                .and_then(|e| e.get("email").and_then(|v| v.as_str()).map(String::from))
                .unwrap_or_default(),
            _ => profile.get("email").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        };

        let provider_user_id = profile.get("id")
            .map(|v| v.to_string())
            .unwrap_or_default();

        let user_info = SsoUserInfo {
            provider: "github".to_string(),
            provider_user_id,
            email: email.clone(),
            email_verified: !email.is_empty(),
            name: profile.get("name").and_then(|v| v.as_str()).map(String::from),
            username: profile.get("login").and_then(|v| v.as_str()).map(String::from),
            avatar_url: profile.get("avatar_url").and_then(|v| v.as_str()).map(String::from),
            groups: vec![],
            raw_profile: profile,
        };

        Ok(TokenExchangeResult {
            access_token,
            refresh_token: None,
            id_token: None,
            expires_in_secs: None,
            user_info,
        })
    }
}

// ── Factory: construct the right provider for a kind ────────────────────────

pub fn build_provider(cfg: OAuth2Config) -> Result<Box<dyn OAuth2Provider>, AuthError> {
    match cfg.kind {
        ProviderKind::Github => Ok(Box::new(GithubProvider::new(cfg))),
        // All OIDC-compliant providers share the same implementation with
        // different issuer URLs (handled via config).
        ProviderKind::Keycloak
        | ProviderKind::Okta
        | ProviderKind::Google
        | ProviderKind::Microsoft
        | ProviderKind::OidcGeneric => Ok(Box::new(OidcProvider::new(cfg))),
    }
}

/// Convenience: preset endpoints for providers with well-known URLs.
pub fn apply_provider_defaults(mut cfg: OAuth2Config) -> OAuth2Config {
    match cfg.kind {
        ProviderKind::Google => {
            if cfg.authorize_url.is_none() {
                cfg.authorize_url = Some("https://accounts.google.com/o/oauth2/v2/auth".to_string());
            }
            if cfg.token_url.is_none() {
                cfg.token_url = Some("https://oauth2.googleapis.com/token".to_string());
            }
            if cfg.userinfo_url.is_none() {
                cfg.userinfo_url = Some("https://openidconnect.googleapis.com/v1/userinfo".to_string());
            }
            if cfg.scopes.is_empty() {
                cfg.scopes = vec!["openid".into(), "profile".into(), "email".into()];
            }
        }
        ProviderKind::Microsoft => {
            // Microsoft/Entra ID "common" tenant — override via issuer_url for single-tenant apps.
            let tenant = cfg.issuer_url.clone().unwrap_or_else(|| "common".to_string());
            if cfg.authorize_url.is_none() {
                cfg.authorize_url = Some(format!(
                    "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
                ));
            }
            if cfg.token_url.is_none() {
                cfg.token_url = Some(format!(
                    "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"
                ));
            }
            if cfg.userinfo_url.is_none() {
                cfg.userinfo_url = Some("https://graph.microsoft.com/oidc/userinfo".to_string());
            }
            if cfg.scopes.is_empty() {
                cfg.scopes = vec!["openid".into(), "profile".into(), "email".into(), "User.Read".into()];
            }
        }
        _ => {
            if cfg.scopes.is_empty() {
                cfg.scopes = vec!["openid".into(), "profile".into(), "email".into()];
            }
        }
    }
    cfg
}

// ── PKCE helpers ────────────────────────────────────────────────────────────

/// Generate a PKCE code verifier (43–128 chars) and its S256 challenge.
pub fn generate_pkce() -> (String, String) {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

/// Generate a random state token for CSRF protection.
pub fn generate_state() -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 24] = rng.gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

// ── Backward-compat shim for the old stub API ──────────────────────────────
// Old code might still reference `SsoProvider` / `NoopSsoProvider`. Keep the
// symbols but mark them deprecated in favor of `OAuth2Provider`.

pub use OAuth2Provider as SsoProvider;

pub struct NoopSsoProvider;

#[async_trait]
impl OAuth2Provider for NoopSsoProvider {
    fn kind(&self) -> ProviderKind { ProviderKind::OidcGeneric }
    fn build_authorize_url(&self, _s: &str, _c: Option<&str>, _n: Option<&str>) -> Result<String, AuthError> {
        Err(AuthError::Internal("SSO not configured".into()))
    }
    async fn exchange_code(&self, _c: &str, _v: Option<&str>) -> Result<TokenExchangeResult, AuthError> {
        Err(AuthError::Internal("SSO not configured".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cfg(kind: ProviderKind) -> OAuth2Config {
        OAuth2Config {
            kind,
            client_id: "client-abc".into(),
            client_secret: "secret-xyz".into(),
            redirect_uri: "https://gw.example.com/cb".into(),
            scopes: vec![],
            issuer_url: None,
            authorize_url: None,
            token_url: None,
            userinfo_url: None,
        }
    }

    // ── ProviderKind round-trip ────────────────────────────────────────

    #[test]
    fn provider_kind_as_str_and_from_str_round_trip() {
        for k in [
            ProviderKind::Keycloak, ProviderKind::Okta, ProviderKind::Google,
            ProviderKind::Github, ProviderKind::Microsoft, ProviderKind::OidcGeneric,
        ] {
            let s = k.as_str();
            assert_eq!(ProviderKind::from_str(s), Some(k));
        }
    }

    #[test]
    fn provider_kind_from_str_rejects_unknown() {
        assert_eq!(ProviderKind::from_str("ldap"), None);
        assert_eq!(ProviderKind::from_str(""), None);
    }

    // ── PKCE / state generation ──────────────────────────────────────

    #[test]
    fn generate_state_produces_unique_high_entropy_strings() {
        let a = generate_state();
        let b = generate_state();
        assert_ne!(a, b);
        // base64 of 24 bytes URL-safe-no-pad is 32 chars
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn generate_pkce_verifier_and_challenge_are_correct_shape() {
        let (verifier, challenge) = generate_pkce();
        // 32 bytes → 43 char URL-safe-no-pad base64
        assert_eq!(verifier.len(), 43);
        assert_eq!(challenge.len(), 43);
        assert_ne!(verifier, challenge);
        // Only URL-safe base64 chars: A-Z a-z 0-9 - _
        assert!(verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert!(challenge.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn generate_pkce_challenge_is_deterministic_sha256_of_verifier() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        use sha2::{Digest, Sha256};
        let (verifier, challenge) = generate_pkce();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
    }

    // ── apply_provider_defaults ──────────────────────────────────────

    #[test]
    fn google_defaults_are_applied() {
        let cfg = apply_provider_defaults(base_cfg(ProviderKind::Google));
        assert_eq!(cfg.authorize_url.as_deref(), Some("https://accounts.google.com/o/oauth2/v2/auth"));
        assert_eq!(cfg.token_url.as_deref(), Some("https://oauth2.googleapis.com/token"));
        assert!(cfg.scopes.contains(&"openid".to_string()));
        assert!(cfg.scopes.contains(&"email".to_string()));
    }

    #[test]
    fn microsoft_defaults_use_common_tenant_when_issuer_unset() {
        let cfg = apply_provider_defaults(base_cfg(ProviderKind::Microsoft));
        assert!(cfg.authorize_url.as_deref().unwrap().contains("/common/oauth2/v2.0/authorize"));
        assert!(cfg.token_url.as_deref().unwrap().contains("/common/oauth2/v2.0/token"));
    }

    #[test]
    fn microsoft_defaults_use_issuer_url_as_tenant() {
        let mut c = base_cfg(ProviderKind::Microsoft);
        c.issuer_url = Some("my-tenant-id".to_string());
        let cfg = apply_provider_defaults(c);
        assert!(cfg.authorize_url.as_deref().unwrap().contains("/my-tenant-id/oauth2"));
    }

    #[test]
    fn explicit_urls_are_preserved_over_defaults() {
        let mut c = base_cfg(ProviderKind::Google);
        c.authorize_url = Some("https://custom.example.com/auth".into());
        let cfg = apply_provider_defaults(c);
        assert_eq!(cfg.authorize_url.as_deref(), Some("https://custom.example.com/auth"));
        // But token_url was unset so still defaulted
        assert_eq!(cfg.token_url.as_deref(), Some("https://oauth2.googleapis.com/token"));
    }

    #[test]
    fn oidc_generic_gets_openid_profile_email_scopes_by_default() {
        let cfg = apply_provider_defaults(base_cfg(ProviderKind::OidcGeneric));
        assert!(cfg.scopes.iter().any(|s| s == "openid"));
    }

    // ── Factory ──────────────────────────────────────────────────────

    #[test]
    fn build_provider_returns_the_right_kind() {
        let p = build_provider(base_cfg(ProviderKind::Github)).unwrap();
        assert_eq!(p.kind(), ProviderKind::Github);
        let p = build_provider(base_cfg(ProviderKind::Keycloak)).unwrap();
        assert_eq!(p.kind(), ProviderKind::Keycloak);
    }

    // ── OIDC authorize URL ───────────────────────────────────────────

    #[test]
    fn oidc_authorize_url_contains_all_required_query_params() {
        let mut c = base_cfg(ProviderKind::Keycloak);
        c.issuer_url = Some("https://kc.example.com/realms/main".into());
        c.scopes = vec!["openid".into(), "profile".into()];
        let p = OidcProvider::new(c);
        let url = p.build_authorize_url("state-123", Some("challenge-abc"), Some("nonce-xyz")).unwrap();
        assert!(url.starts_with("https://kc.example.com/realms/main/protocol/openid-connect/auth?"));
        assert!(url.contains("state=state-123"));
        assert!(url.contains("code_challenge=challenge-abc"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("nonce=nonce-xyz"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=client-abc"));
        // Scopes URL-encoded (space → %20)
        assert!(url.contains("scope=openid%20profile"));
    }

    #[test]
    fn oidc_authorize_url_errors_if_no_issuer_and_no_authorize_url() {
        let p = OidcProvider::new(base_cfg(ProviderKind::OidcGeneric));
        let err = p.build_authorize_url("s", None, None);
        assert!(err.is_err());
    }

    // ── GitHub authorize URL ────────────────────────────────────────

    #[test]
    fn github_authorize_url_points_to_github_and_has_state() {
        let mut c = base_cfg(ProviderKind::Github);
        c.scopes = vec!["read:user".into()];
        let p = GithubProvider::new(c);
        let url = p.build_authorize_url("csrf-xyz", None, None).unwrap();
        assert!(url.starts_with("https://github.com/login/oauth/authorize?"));
        assert!(url.contains("state=csrf-xyz"));
        assert!(url.contains("client_id=client-abc"));
        assert!(url.contains("allow_signup=true"));
        // GitHub OAuth doesn't use PKCE — no code_challenge even if one is passed.
        let url2 = p.build_authorize_url("x", Some("challenge"), None).unwrap();
        assert!(!url2.contains("code_challenge"));
    }

    #[test]
    fn github_provider_supplies_default_scopes_if_empty() {
        let c = base_cfg(ProviderKind::Github); // scopes empty
        let p = GithubProvider::new(c);
        let url = p.build_authorize_url("s", None, None).unwrap();
        // Default scopes: read:user user:email → URL-encoded
        assert!(url.contains("read%3Auser"));
        assert!(url.contains("user%3Aemail"));
    }
}
