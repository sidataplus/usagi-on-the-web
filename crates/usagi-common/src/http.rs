use crate::error::{ErrorCode, ErrorEnvelope, UsagiError};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

pub const API_KEY_HEADER: &str = "x-api-key";
pub const DEFAULT_API_BODY_LIMIT_BYTES: usize = 262_144;

pub struct ApiProductionBootConfig<'a> {
    pub environment: Option<&'a str>,
    pub auth_mode: Option<&'a str>,
    pub shared_secret: Option<&'a str>,
    pub jobs_db_path: Option<&'a str>,
    pub artifact_paths: Vec<(&'a str, Option<&'a str>)>,
}

#[derive(Debug, Clone, Copy)]
pub struct SignedAuthConfig<'a> {
    pub secret: &'a str,
    pub now: DateTime<Utc>,
    pub allowed_clock_skew_seconds: i64,
    pub nonce_ttl_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct SignedRequest<'a> {
    pub method: &'a str,
    pub path_with_query: &'a str,
    pub headers: BTreeMap<String, String>,
    pub body: &'a [u8],
}

#[derive(Debug, Default)]
pub struct SignatureNonceCache {
    seen: HashMap<String, DateTime<Utc>>,
}

pub fn api_body_limit_bytes() -> usize {
    api_body_limit_bytes_from_env_value(std::env::var("USAGI_API_BODY_LIMIT_BYTES").ok().as_deref())
}

pub fn api_body_limit_bytes_from_env_value(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(DEFAULT_API_BODY_LIMIT_BYTES)
}

pub fn api_production_boot_errors(config: ApiProductionBootConfig<'_>) -> Vec<String> {
    if !matches!(config.environment.map(str::trim), Some("production")) {
        return Vec::new();
    }

    let mut errors = Vec::new();
    if config.auth_mode.map(str::trim) != Some("signed") {
        errors.push("USAGI_API_AUTH_MODE must be signed in production".to_string());
    }
    if is_blank(config.shared_secret) {
        errors.push("USAGI_API_SHARED_SECRET is required in production".to_string());
    }
    if is_blank(config.jobs_db_path) {
        errors.push("JOBS_DB_PATH is required in production".to_string());
    }
    for (key, value) in config.artifact_paths {
        if is_blank(value) {
            errors.push(format!("{key} is required in production"));
        }
    }

    errors
}

pub fn api_production_boot_errors_from_env(artifact_path_keys: &[&str]) -> Vec<String> {
    let environment = std::env::var("USAGI_API_ENV").ok();
    let auth_mode = std::env::var("USAGI_API_AUTH_MODE").ok();
    let shared_secret = std::env::var("USAGI_API_SHARED_SECRET").ok();
    let jobs_db_path = std::env::var("JOBS_DB_PATH").ok();
    let artifact_values: Vec<(&str, Option<String>)> = artifact_path_keys
        .iter()
        .map(|key| (*key, std::env::var(key).ok()))
        .collect();
    let artifact_paths = artifact_values
        .iter()
        .map(|(key, value)| (*key, value.as_deref()))
        .collect();

    api_production_boot_errors(ApiProductionBootConfig {
        environment: environment.as_deref(),
        auth_mode: auth_mode.as_deref(),
        shared_secret: shared_secret.as_deref(),
        jobs_db_path: jobs_db_path.as_deref(),
        artifact_paths,
    })
}

fn is_blank(value: Option<&str>) -> bool {
    value.map(str::trim).unwrap_or_default().is_empty()
}

pub fn verify_signed_request(
    config: SignedAuthConfig<'_>,
    request: SignedRequest<'_>,
    nonce_cache: &mut SignatureNonceCache,
) -> Result<(), UsagiError> {
    let request_id = required_header(&request, "x-request-id")?;
    let service = required_header(&request, "x-usagi-service")?;
    let timestamp_header = required_header(&request, "x-usagi-timestamp")?;
    let nonce = required_header(&request, "x-usagi-nonce")?;
    let content_sha = required_header(&request, "x-usagi-content-sha256")?;
    let signature = required_header(&request, "x-usagi-signature")?;

    if service != "rails" {
        return Err(signature_error(
            ErrorCode::SignatureInvalid,
            "signed request service is not allowed",
            request_id,
        ));
    }

    let timestamp = DateTime::parse_from_rfc3339(timestamp_header)
        .map_err(|_| {
            signature_error(
                ErrorCode::SignatureTimestampInvalid,
                "Request timestamp is invalid",
                request_id,
            )
        })?
        .with_timezone(&Utc);
    let skew = (config.now - timestamp).num_seconds().abs();
    if skew > config.allowed_clock_skew_seconds {
        return Err(signature_error(
            ErrorCode::SignatureTimestampInvalid,
            "Request timestamp is outside the allowed window",
            request_id,
        ));
    }

    if nonce_cache.contains(nonce, config.now) {
        return Err(signature_error(
            ErrorCode::SignatureNonceReplayed,
            "Request nonce has already been used",
            request_id,
        ));
    }

    let actual_content_sha = sha256_hex(request.body);
    if !constant_time_eq(actual_content_sha.as_bytes(), content_sha.as_bytes()) {
        return Err(signature_error(
            ErrorCode::SignatureBodyHashMismatch,
            "Request body hash does not match signature header",
            request_id,
        ));
    }

    if !signature.starts_with("v1=") {
        return Err(signature_error(
            ErrorCode::SignatureVersionUnsupported,
            "Request signature version is unsupported",
            request_id,
        ));
    }

    let expected_signature = sign_request_v1(
        config.secret,
        request.method,
        request.path_with_query,
        timestamp_header,
        nonce,
        request_id,
        content_sha,
    );
    let expected_bytes = expected_signature.as_bytes();
    let actual_bytes = signature.as_bytes();
    if !constant_time_eq(expected_bytes, actual_bytes) {
        return Err(signature_error(
            ErrorCode::SignatureInvalid,
            "Request signature is invalid",
            request_id,
        ));
    }

    nonce_cache.store(
        nonce.to_string(),
        config.now + chrono::Duration::seconds(config.nonce_ttl_seconds.max(1)),
    );
    Ok(())
}

pub fn sign_request_v1(
    secret: &str,
    method: &str,
    path_with_query: &str,
    timestamp: &str,
    nonce: &str,
    request_id: &str,
    content_sha: &str,
) -> String {
    let canonical = [
        method.to_ascii_uppercase(),
        path_with_query.to_string(),
        timestamp.to_string(),
        nonce.to_string(),
        request_id.to_string(),
        content_sha.to_string(),
    ]
    .join("\n");
    let digest = hmac_sha256(secret.as_bytes(), canonical.as_bytes());
    format!("v1={}", general_purpose::STANDARD.encode(digest))
}

pub fn sha256_hex(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl SignatureNonceCache {
    fn contains(&mut self, nonce: &str, now: DateTime<Utc>) -> bool {
        self.seen.retain(|_, expires_at| *expires_at > now);
        self.seen.contains_key(nonce)
    }

    fn store(&mut self, nonce: String, expires_at: DateTime<Utc>) {
        self.seen.insert(nonce, expires_at);
    }
}

fn required_header<'a>(request: &'a SignedRequest<'_>, name: &str) -> Result<&'a str, UsagiError> {
    request
        .headers
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            signature_error(
                ErrorCode::SignatureRequired,
                format!("{name} header is required"),
                request
                    .headers
                    .get("x-request-id")
                    .map(String::as_str)
                    .unwrap_or(""),
            )
        })
}

fn signature_error(code: ErrorCode, message: impl Into<String>, request_id: &str) -> UsagiError {
    let error = UsagiError::new(code, message);
    if request_id.is_empty() {
        error
    } else {
        error.with_request_id(request_id)
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0_u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut outer = [0x5c_u8; BLOCK_SIZE];
    let mut inner = [0x36_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        outer[index] ^= key_block[index];
        inner[index] ^= key_block[index];
    }

    let mut inner_hash = Sha256::new();
    inner_hash.update(inner);
    inner_hash.update(message);
    let inner_digest = inner_hash.finalize();

    let mut outer_hash = Sha256::new();
    outer_hash.update(outer);
    outer_hash.update(inner_digest);
    outer_hash.finalize().into()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let l = left.get(index).copied().unwrap_or_default();
        let r = right.get(index).copied().unwrap_or_default();
        diff |= usize::from(l ^ r);
    }
    diff == 0
}

pub fn rewrite_error_envelope_request_id(body: &[u8], request_id: &str) -> Option<Vec<u8>> {
    let mut value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let error = value.get_mut("error")?.as_object_mut()?;
    error.insert(
        "request_id".to_string(),
        serde_json::Value::String(request_id.to_string()),
    );
    serde_json::to_vec(&value).ok()
}

pub fn error_envelope_body(body: &[u8], request_id: &str, fallback_code: ErrorCode) -> Vec<u8> {
    if let Some(updated) = rewrite_error_envelope_request_id(body, request_id) {
        return updated;
    }
    let message = std::str::from_utf8(body)
        .ok()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or("request failed");
    serde_json::to_vec(&ErrorEnvelope::from(
        UsagiError::new(fallback_code, message).with_request_id(request_id),
    ))
    .unwrap_or_else(|_| {
        format!(
            r#"{{"error":{{"code":"{}","message":"request failed","request_id":"{}"}}}}"#,
            fallback_code.as_str(),
            request_id
        )
        .into_bytes()
    })
}

pub fn api_key_is_authorized(
    configured_keys: &str,
    x_api_key: Option<&str>,
    authorization: Option<&str>,
) -> bool {
    let configured: Vec<&str> = configured_keys
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .collect();
    if configured.is_empty() {
        return false;
    }
    let presented = x_api_key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .or_else(|| bearer_token(authorization));
    presented
        .map(|key| configured.contains(&key))
        .unwrap_or(false)
}

pub fn is_public_probe_path(path: &str) -> bool {
    let mut parts = path.trim_matches('/').split('/');
    let Some(_service) = parts.next() else {
        return false;
    };
    let Some(endpoint) = parts.next() else {
        return false;
    };
    parts.next().is_none() && matches!(endpoint, "health" | "status")
}

pub fn accepts_jsonl(accept_header: Option<&str>) -> bool {
    accept_header.is_some_and(|value| {
        value.split(',').any(|part| {
            let media_type = part.trim().split(';').next().unwrap_or("").trim();
            media_type.eq_ignore_ascii_case("application/jsonl")
                || media_type.eq_ignore_ascii_case("application/x-ndjson")
        })
    })
}

fn bearer_token(authorization: Option<&str>) -> Option<&str> {
    authorization?
        .trim()
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::{TimeZone, Utc};

    use super::{
        accepts_jsonl, api_body_limit_bytes_from_env_value, api_key_is_authorized,
        api_production_boot_errors, error_envelope_body, is_public_probe_path,
        rewrite_error_envelope_request_id, sha256_hex, sign_request_v1, verify_signed_request,
        ApiProductionBootConfig, SignatureNonceCache, SignedAuthConfig, SignedRequest,
    };
    use crate::error::ErrorCode;

    #[test]
    fn rewrites_error_envelope_request_id() {
        let body =
            br#"{"error":{"code":"INDEX_NOT_READY","message":"missing","request_id":"old"}}"#;

        let rewritten = rewrite_error_envelope_request_id(body, "req_expected").unwrap();
        let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();

        assert_eq!(value["error"]["request_id"], "req_expected");
    }

    #[test]
    fn ignores_non_error_json() {
        assert!(rewrite_error_envelope_request_id(br#"{"status":"ok"}"#, "req").is_none());
    }

    #[test]
    fn wraps_plain_text_error_body() {
        let wrapped = error_envelope_body(
            b"Failed to parse the request body as JSON",
            "req_expected",
            ErrorCode::BadRequest,
        );
        let value: serde_json::Value = serde_json::from_slice(&wrapped).unwrap();

        assert_eq!(value["error"]["code"], "BAD_REQUEST");
        assert_eq!(value["error"]["request_id"], "req_expected");
        assert_eq!(
            value["error"]["message"],
            "Failed to parse the request body as JSON"
        );
    }

    #[test]
    fn api_key_auth_fails_closed_when_no_keys_are_configured() {
        assert!(!api_key_is_authorized("", None, None));
        assert!(!api_key_is_authorized(" , ", None, None));
    }

    #[test]
    fn api_key_auth_accepts_header_or_bearer_token() {
        assert!(api_key_is_authorized("alpha,beta", Some("beta"), None));
        assert!(api_key_is_authorized(
            "alpha,beta",
            None,
            Some("Bearer alpha")
        ));
    }

    #[test]
    fn api_key_auth_rejects_missing_or_wrong_keys_when_configured() {
        assert!(!api_key_is_authorized("alpha", None, None));
        assert!(!api_key_is_authorized("alpha", Some("wrong"), None));
        assert!(!api_key_is_authorized("alpha", None, Some("Basic alpha")));
    }

    #[test]
    fn health_and_status_paths_are_public_probes() {
        assert!(is_public_probe_path("/search/health"));
        assert!(is_public_probe_path("/mapper/status"));
        assert!(!is_public_probe_path("/search/concepts"));
        assert!(!is_public_probe_path("/jobs/job_1/status"));
    }

    #[test]
    fn jsonl_accept_header_supports_rails_result_streaming() {
        assert!(accepts_jsonl(Some("application/jsonl")));
        assert!(accepts_jsonl(Some(
            "application/json, application/x-ndjson; charset=utf-8"
        )));
        assert!(!accepts_jsonl(None));
        assert!(!accepts_jsonl(Some("application/json")));
    }

    #[test]
    fn api_body_limit_defaults_to_tight_json_payload_cap() {
        assert_eq!(api_body_limit_bytes_from_env_value(None), 262_144);
        assert_eq!(api_body_limit_bytes_from_env_value(Some("")), 262_144);
        assert_eq!(api_body_limit_bytes_from_env_value(Some("nope")), 262_144);
    }

    #[test]
    fn api_body_limit_accepts_positive_env_override() {
        assert_eq!(api_body_limit_bytes_from_env_value(Some("1024")), 1024);
        assert_eq!(api_body_limit_bytes_from_env_value(Some("0")), 262_144);
    }

    #[test]
    fn api_production_boot_check_rejects_disabled_auth_and_missing_secret() {
        let errors = api_production_boot_errors(ApiProductionBootConfig {
            environment: Some("production"),
            auth_mode: Some("disabled"),
            shared_secret: None,
            jobs_db_path: None,
            artifact_paths: vec![("CATALOG_DB_PATH", None)],
        });

        assert!(errors.contains(&"USAGI_API_AUTH_MODE must be signed in production".to_string()));
        assert!(errors.contains(&"USAGI_API_SHARED_SECRET is required in production".to_string()));
        assert!(errors.contains(&"JOBS_DB_PATH is required in production".to_string()));
        assert!(errors.contains(&"CATALOG_DB_PATH is required in production".to_string()));
    }

    #[test]
    fn api_production_boot_check_allows_non_production_defaults() {
        let errors = api_production_boot_errors(ApiProductionBootConfig {
            environment: Some("development"),
            auth_mode: Some("disabled"),
            shared_secret: None,
            jobs_db_path: None,
            artifact_paths: vec![("CATALOG_DB_PATH", None)],
        });

        assert!(errors.is_empty());
    }

    #[test]
    fn signed_auth_accepts_valid_request_once() {
        let now = Utc.with_ymd_and_hms(2026, 5, 31, 12, 0, 0).unwrap();
        let body = br#"{"q":"diabetes"}"#;
        let content_sha = sha256_hex(body);
        let signature = sign_request_v1(
            "secret",
            "POST",
            "/search/concepts",
            "2026-05-31T12:00:00Z",
            "nonce_one",
            "req_one",
            &content_sha,
        );
        assert_eq!(
            content_sha,
            "29f5c2dcaedabbd63e662149fc4c5aee1ad7fce11387ae2e13ecba96c57b1f7e"
        );
        assert_eq!(signature, "v1=wkeBw6hVndTrAdDIgWCjw58AVjar5QKn0mxq0mmE2fY=");
        let request = signed_request(
            "POST",
            "/search/concepts",
            body,
            [
                ("x-request-id", "req_one"),
                ("x-usagi-service", "rails"),
                ("x-usagi-timestamp", "2026-05-31T12:00:00Z"),
                ("x-usagi-nonce", "nonce_one"),
                ("x-usagi-content-sha256", &content_sha),
                ("x-usagi-signature", &signature),
            ],
        );
        let config = SignedAuthConfig {
            secret: "secret",
            now,
            allowed_clock_skew_seconds: 300,
            nonce_ttl_seconds: 300,
        };
        let mut nonce_cache = SignatureNonceCache::default();

        verify_signed_request(config, request, &mut nonce_cache).unwrap();
    }

    #[test]
    fn signed_auth_rejects_missing_signature() {
        let now = Utc.with_ymd_and_hms(2026, 5, 31, 12, 0, 0).unwrap();
        let body = b"{}";
        let content_sha = sha256_hex(body);
        let request = signed_request(
            "POST",
            "/search/concepts",
            body,
            [
                ("x-request-id", "req_one"),
                ("x-usagi-service", "rails"),
                ("x-usagi-timestamp", "2026-05-31T12:00:00Z"),
                ("x-usagi-nonce", "nonce_one"),
                ("x-usagi-content-sha256", &content_sha),
            ],
        );
        let mut nonce_cache = SignatureNonceCache::default();
        let err = verify_signed_request(
            SignedAuthConfig {
                secret: "secret",
                now,
                allowed_clock_skew_seconds: 300,
                nonce_ttl_seconds: 300,
            },
            request,
            &mut nonce_cache,
        )
        .unwrap_err();

        assert_eq!(err.code(), ErrorCode::SignatureRequired);
    }

    #[test]
    fn signed_auth_rejects_bad_body_hash_bad_signature_expired_timestamp_and_replay() {
        let now = Utc.with_ymd_and_hms(2026, 5, 31, 12, 0, 0).unwrap();
        let body = br#"{"q":"diabetes"}"#;
        let content_sha = sha256_hex(body);
        let config = SignedAuthConfig {
            secret: "secret",
            now,
            allowed_clock_skew_seconds: 300,
            nonce_ttl_seconds: 300,
        };

        let bad_hash = signed_request(
            "POST",
            "/search/concepts",
            body,
            signed_headers(
                "req_hash",
                "nonce_hash",
                "2026-05-31T12:00:00Z",
                "00",
                "v1=bad",
            ),
        );
        let mut nonce_cache = SignatureNonceCache::default();
        let err = verify_signed_request(config, bad_hash, &mut nonce_cache).unwrap_err();
        assert_eq!(err.code(), ErrorCode::SignatureBodyHashMismatch);

        let bad_signature = signed_request(
            "POST",
            "/search/concepts",
            body,
            signed_headers(
                "req_bad_sig",
                "nonce_bad_sig",
                "2026-05-31T12:00:00Z",
                &content_sha,
                "v1=bad",
            ),
        );
        let err = verify_signed_request(config, bad_signature, &mut nonce_cache).unwrap_err();
        assert_eq!(err.code(), ErrorCode::SignatureInvalid);

        let old_signature = sign_request_v1(
            "secret",
            "POST",
            "/search/concepts",
            "2026-05-31T11:50:00Z",
            "nonce_old",
            "req_old",
            &content_sha,
        );
        let expired = signed_request(
            "POST",
            "/search/concepts",
            body,
            signed_headers(
                "req_old",
                "nonce_old",
                "2026-05-31T11:50:00Z",
                &content_sha,
                &old_signature,
            ),
        );
        let err = verify_signed_request(config, expired, &mut nonce_cache).unwrap_err();
        assert_eq!(err.code(), ErrorCode::SignatureTimestampInvalid);

        let signature = sign_request_v1(
            "secret",
            "POST",
            "/search/concepts",
            "2026-05-31T12:00:00Z",
            "nonce_replay",
            "req_replay",
            &content_sha,
        );
        let replay = signed_request(
            "POST",
            "/search/concepts",
            body,
            signed_headers(
                "req_replay",
                "nonce_replay",
                "2026-05-31T12:00:00Z",
                &content_sha,
                &signature,
            ),
        );
        verify_signed_request(config, replay.clone(), &mut nonce_cache).unwrap();
        let err = verify_signed_request(config, replay, &mut nonce_cache).unwrap_err();
        assert_eq!(err.code(), ErrorCode::SignatureNonceReplayed);
    }

    fn signed_request<const N: usize>(
        method: &'static str,
        path_with_query: &'static str,
        body: &'static [u8],
        headers: [(&'static str, &str); N],
    ) -> SignedRequest<'static> {
        SignedRequest {
            method,
            path_with_query,
            headers: headers
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<BTreeMap<_, _>>(),
            body,
        }
    }

    fn signed_headers<'a>(
        request_id: &'a str,
        nonce: &'a str,
        timestamp: &'a str,
        content_sha: &'a str,
        signature: &'a str,
    ) -> [(&'static str, &'a str); 6] {
        [
            ("x-request-id", request_id),
            ("x-usagi-service", "rails"),
            ("x-usagi-timestamp", timestamp),
            ("x-usagi-nonce", nonce),
            ("x-usagi-content-sha256", content_sha),
            ("x-usagi-signature", signature),
        ]
    }
}
