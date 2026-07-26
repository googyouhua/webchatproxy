use axum::http::HeaderMap;
use uuid::Uuid;

pub fn generate_token() -> String {
    Uuid::new_v4().to_string()
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let header = headers.get("Authorization")?;
    let value = header.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|s| s.to_string())
}

pub fn validate_token(token: &str, expected: &str) -> bool {
    token == expected
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    #[test]
    fn test_generate_token_is_non_empty() {
        let t = super::generate_token();
        assert!(!t.is_empty(), "token should not be empty");
    }

    #[test]
    fn test_generate_token_is_unique() {
        let a = super::generate_token();
        let b = super::generate_token();
        assert_ne!(a, b, "tokens should be unique");
    }

    #[test]
    fn test_valid_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer test-token-123".parse().unwrap());
        assert_eq!(super::extract_bearer_token(&headers), Some("test-token-123".into()));
    }

    #[test]
    fn test_missing_auth_header() {
        let headers = HeaderMap::new();
        assert_eq!(super::extract_bearer_token(&headers), None);
    }

    #[test]
    fn test_wrong_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Basic dXNlcjpwYXNz".parse().unwrap());
        assert_eq!(super::extract_bearer_token(&headers), None);
    }

    #[test]
    fn test_validate_token_ok() {
        assert!(super::validate_token("abc", "abc"));
    }

    #[test]
    fn test_validate_token_fail() {
        assert!(!super::validate_token("abc", "xyz"));
    }
}
