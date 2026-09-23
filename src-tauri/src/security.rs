use std::{error::Error, fmt};

use keyring::{Entry, Error as KeyringError};

const KEYRING_SERVICE: &str = "io.github.notsonata.refrain";
const SPOTIFY_REFRESH_TOKEN_ACCOUNT: &str = "spotify-refresh-token";

pub trait RefreshTokenStore: Send + Sync {
    fn get_refresh_token(&self) -> Result<Option<String>, CredentialStoreError>;
    fn set_refresh_token(&self, token: &str) -> Result<(), CredentialStoreError>;
    fn clear_refresh_token(&self) -> Result<(), CredentialStoreError>;
}

#[derive(Debug, Default)]
pub struct KeyringRefreshTokenStore;

impl KeyringRefreshTokenStore {
    fn entry(&self) -> Result<Entry, CredentialStoreError> {
        Entry::new(KEYRING_SERVICE, SPOTIFY_REFRESH_TOKEN_ACCOUNT)
            .map_err(CredentialStoreError::from)
    }
}

impl RefreshTokenStore for KeyringRefreshTokenStore {
    fn get_refresh_token(&self) -> Result<Option<String>, CredentialStoreError> {
        match self.entry()?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(CredentialStoreError::from(error)),
        }
    }

    fn set_refresh_token(&self, token: &str) -> Result<(), CredentialStoreError> {
        self.entry()?
            .set_password(token)
            .map_err(CredentialStoreError::from)
    }

    fn clear_refresh_token(&self) -> Result<(), CredentialStoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(CredentialStoreError::from(error)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialStoreError {
    message: String,
}

impl CredentialStoreError {
    #[cfg(test)]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CredentialStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CredentialStoreError {}

impl From<KeyringError> for CredentialStoreError {
    fn from(error: KeyringError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}
