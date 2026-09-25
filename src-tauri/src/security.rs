use std::{error::Error, fmt};

use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};

const KEYRING_SERVICE: &str = "io.github.notsonata.refrain";
const SPOTIFY_REFRESH_TOKEN_ACCOUNT: &str = "spotify-refresh-token";
const SOULSEEK_CREDENTIALS_ACCOUNT: &str = "soulseek-credentials";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoulseekCredentials {
    pub username: String,
    pub password: String,
}

pub trait SoulseekCredentialStore: Send + Sync {
    fn get_credentials(&self) -> Result<Option<SoulseekCredentials>, CredentialStoreError>;
    fn set_credentials(
        &self,
        credentials: &SoulseekCredentials,
    ) -> Result<(), CredentialStoreError>;
    fn clear_credentials(&self) -> Result<(), CredentialStoreError>;
}

#[derive(Debug, Default)]
pub struct KeyringSoulseekCredentialStore;

impl KeyringSoulseekCredentialStore {
    fn entry(&self) -> Result<Entry, CredentialStoreError> {
        Entry::new(KEYRING_SERVICE, SOULSEEK_CREDENTIALS_ACCOUNT)
            .map_err(CredentialStoreError::from)
    }
}

impl SoulseekCredentialStore for KeyringSoulseekCredentialStore {
    fn get_credentials(&self) -> Result<Option<SoulseekCredentials>, CredentialStoreError> {
        let serialized = match self.entry()?.get_password() {
            Ok(value) => value,
            Err(KeyringError::NoEntry) => return Ok(None),
            Err(error) => return Err(CredentialStoreError::from(error)),
        };

        serde_json::from_str(&serialized)
            .map(Some)
            .map_err(|error| CredentialStoreError::new(error.to_string()))
    }

    fn set_credentials(
        &self,
        credentials: &SoulseekCredentials,
    ) -> Result<(), CredentialStoreError> {
        let serialized = serde_json::to_string(credentials)
            .map_err(|error| CredentialStoreError::new(error.to_string()))?;
        self.entry()?
            .set_password(&serialized)
            .map_err(CredentialStoreError::from)
    }

    fn clear_credentials(&self) -> Result<(), CredentialStoreError> {
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
