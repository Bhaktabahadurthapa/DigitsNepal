//! API keys live in the macOS Keychain, never in settings files, logs, or
//! the webview. The frontend can only ask whether a key exists.

pub const SERVICE: &str = "org.digitsnepal.app";

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error("keychain error: {0}")]
    Backend(String),
}

/// Basic shape check so obvious paste mistakes are caught locally.
pub fn validate_api_key_shape(key: &str) -> Result<&str, String> {
    let k = key.trim();
    if k.is_empty() {
        return Err("The API key is empty.".into());
    }
    if k.len() < 8 || k.len() > 400 {
        return Err("That does not look like an API key (unexpected length).".into());
    }
    if !k
        .chars()
        .all(|c| c.is_ascii_graphic() && c != '"' && c != '\'' && c != '`')
    {
        return Err("The API key contains unexpected characters (spaces or quotes?).".into());
    }
    Ok(k)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use keyring::Entry;

    fn entry(account: &str) -> Result<Entry, KeychainError> {
        Entry::new(SERVICE, account).map_err(|e| KeychainError::Backend(e.to_string()))
    }
    pub fn set(account: &str, secret: &str) -> Result<(), KeychainError> {
        entry(account)?
            .set_password(secret)
            .map_err(|e| KeychainError::Backend(e.to_string()))
    }
    pub fn get(account: &str) -> Result<Option<String>, KeychainError> {
        match entry(account)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeychainError::Backend(e.to_string())),
        }
    }
    pub fn delete(account: &str) -> Result<(), KeychainError> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(KeychainError::Backend(e.to_string())),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    pub fn set(_: &str, _: &str) -> Result<(), KeychainError> {
        Err(KeychainError::Backend("Keychain is only supported on macOS".into()))
    }
    pub fn get(_: &str) -> Result<Option<String>, KeychainError> {
        Ok(None)
    }
    pub fn delete(_: &str) -> Result<(), KeychainError> {
        Ok(())
    }
}

pub use imp::{delete, get, set};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_shape() {
        assert!(validate_api_key_shape("").is_err());
        assert!(validate_api_key_shape("short").is_err());
        assert!(validate_api_key_shape("\"sk-quoted-key-123\"").is_err());
        assert!(validate_api_key_shape("sk-ant-api03-abc.DEF_123").is_ok());
        assert!(validate_api_key_shape("AIzaSy has spaces in it 12345").is_err());
        assert_eq!(
            validate_api_key_shape("  AIzaSyA1234567890abcdefghijklmnop_-  ").unwrap(),
            "AIzaSyA1234567890abcdefghijklmnop_-"
        );
    }
}
