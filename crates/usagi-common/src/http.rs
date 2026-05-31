use crate::error::{ErrorCode, ErrorEnvelope, UsagiError};

pub const API_KEY_HEADER: &str = "x-api-key";
pub const DEFAULT_API_BODY_LIMIT_BYTES: usize = 262_144;

pub struct ApiProductionBootConfig<'a> {
    pub environment: Option<&'a str>,
    pub auth_mode: Option<&'a str>,
    pub shared_secret: Option<&'a str>,
    pub jobs_db_path: Option<&'a str>,
    pub artifact_paths: Vec<(&'a str, Option<&'a str>)>,
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
    use super::{
        accepts_jsonl, api_body_limit_bytes_from_env_value, api_key_is_authorized,
        api_production_boot_errors, error_envelope_body, is_public_probe_path,
        rewrite_error_envelope_request_id, ApiProductionBootConfig,
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
}
