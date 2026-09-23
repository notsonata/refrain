use std::{
    error::Error,
    fmt,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::security::{CredentialStoreError, KeyringRefreshTokenStore, RefreshTokenStore};

const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const CALLBACK_PATH: &str = "/callback";
const REGISTERED_REDIRECT_URI: &str = "http://127.0.0.1/callback";
const SPOTIFY_SCOPES: &str = "user-library-read playlist-read-private playlist-read-collaborative";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const ACCESS_TOKEN_REFRESH_SKEW: Duration = Duration::from_secs(30);

pub struct SpotifyClient {
    token_client: Arc<dyn TokenClient>,
    credentials: Arc<dyn RefreshTokenStore>,
    browser: Arc<dyn BrowserOpener>,
    access_token: Mutex<Option<CachedAccessToken>>,
    callback_timeout: Duration,
}

impl SpotifyClient {
    pub fn new() -> Result<Self, SpotifyAuthError> {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| {
                SpotifyAuthError::new(
                    "httpClientFailed",
                    format!("Failed to initialize Spotify HTTP client: {error}"),
                )
            })?;

        Ok(Self::with_dependencies(
            Arc::new(ReqwestTokenClient::new(http_client, TOKEN_URL)),
            Arc::new(KeyringRefreshTokenStore),
            Arc::new(SystemBrowser),
            CALLBACK_TIMEOUT,
        ))
    }

    fn with_dependencies(
        token_client: Arc<dyn TokenClient>,
        credentials: Arc<dyn RefreshTokenStore>,
        browser: Arc<dyn BrowserOpener>,
        callback_timeout: Duration,
    ) -> Self {
        Self {
            token_client,
            credentials,
            browser,
            access_token: Mutex::new(None),
            callback_timeout,
        }
    }

    pub fn normalize_client_id(value: &str) -> Result<String, SpotifyAuthError> {
        let client_id = value.trim();
        if client_id.is_empty()
            || client_id.len() > 128
            || client_id.chars().any(char::is_whitespace)
        {
            return Err(SpotifyAuthError::new(
                "invalidClientId",
                "Enter the Client ID from your Spotify developer application.",
            ));
        }

        Ok(client_id.to_owned())
    }

    pub fn status(&self, client_id: Option<String>) -> Result<SpotifyAuthStatus, SpotifyAuthError> {
        let connected = self
            .credentials
            .get_refresh_token()
            .map_err(credential_error)?
            .is_some();
        let access_token_cached = self
            .access_token
            .lock()
            .map_err(|_| lock_error())?
            .as_ref()
            .is_some_and(CachedAccessToken::is_current);

        Ok(SpotifyAuthStatus {
            client_id,
            connected,
            access_token_cached,
            registered_redirect_uri: REGISTERED_REDIRECT_URI,
        })
    }

    pub fn connect(&self, client_id: &str) -> Result<(), SpotifyAuthError> {
        let client_id = Self::normalize_client_id(client_id)?;
        let callback = LoopbackCallback::bind(self.callback_timeout)?;
        let verifier = random_urlsafe(32)?;
        let state = random_urlsafe(32)?;
        let challenge = pkce_challenge(&verifier);
        let authorization_url =
            build_authorization_url(&client_id, callback.redirect_uri(), &state, &challenge)?;

        self.browser.open(&authorization_url)?;
        let code = callback.receive(&state)?;
        let token = self
            .token_client
            .exchange_code(&client_id, &code, callback.redirect_uri(), &verifier)
            .map_err(|error| map_token_error(error, false))?;
        let refresh_token = token.refresh_token.as_deref().ok_or_else(|| {
            SpotifyAuthError::new(
                "authorizationFailed",
                "Spotify did not return a refresh credential.",
            )
        })?;

        self.credentials
            .set_refresh_token(refresh_token)
            .map_err(credential_error)?;
        self.cache_access_token(&token)?;

        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), SpotifyAuthError> {
        self.access_token.lock().map_err(|_| lock_error())?.take();
        self.credentials
            .clear_refresh_token()
            .map_err(credential_error)
    }

    #[allow(dead_code)]
    pub fn access_token(&self, client_id: &str) -> Result<String, SpotifyAuthError> {
        let client_id = Self::normalize_client_id(client_id)?;

        if let Some(token) = self
            .access_token
            .lock()
            .map_err(|_| lock_error())?
            .as_ref()
            .filter(|token| token.is_current())
        {
            return Ok(token.value.clone());
        }

        let refresh_token = self
            .credentials
            .get_refresh_token()
            .map_err(credential_error)?
            .ok_or_else(|| {
                SpotifyAuthError::new(
                    "notConnected",
                    "Connect Spotify before accessing Spotify data.",
                )
            })?;

        let token = match self.token_client.refresh(&client_id, &refresh_token) {
            Ok(token) => token,
            Err(TokenRequestError::OAuth { code, .. }) if code == "invalid_grant" => {
                self.credentials
                    .clear_refresh_token()
                    .map_err(credential_error)?;
                return Err(SpotifyAuthError::new(
                    "reauthorizationRequired",
                    "Spotify authorization expired or was revoked. Connect Spotify again.",
                ));
            }
            Err(error) => return Err(map_token_error(error, true)),
        };

        if let Some(new_refresh_token) = token.refresh_token.as_deref() {
            self.credentials
                .set_refresh_token(new_refresh_token)
                .map_err(credential_error)?;
        }

        let value = token.access_token.clone();
        self.cache_access_token(&token)?;
        Ok(value)
    }

    fn cache_access_token(&self, token: &TokenResponse) -> Result<(), SpotifyAuthError> {
        let refresh_after = Instant::now()
            + Duration::from_secs(token.expires_in).saturating_sub(ACCESS_TOKEN_REFRESH_SKEW);
        self.access_token
            .lock()
            .map_err(|_| lock_error())?
            .replace(CachedAccessToken {
                value: token.access_token.clone(),
                refresh_after,
            });
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyAuthStatus {
    pub client_id: Option<String>,
    pub connected: bool,
    pub access_token_cached: bool,
    pub registered_redirect_uri: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyAuthError {
    pub code: String,
    pub message: String,
}

impl SpotifyAuthError {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for SpotifyAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SpotifyAuthError {}

struct CachedAccessToken {
    value: String,
    refresh_after: Instant,
}

impl CachedAccessToken {
    fn is_current(&self) -> bool {
        Instant::now() < self.refresh_after
    }
}

trait BrowserOpener: Send + Sync {
    fn open(&self, url: &Url) -> Result<(), SpotifyAuthError>;
}

struct SystemBrowser;

impl BrowserOpener for SystemBrowser {
    fn open(&self, url: &Url) -> Result<(), SpotifyAuthError> {
        open::that_detached(url.as_str()).map_err(|error| {
            SpotifyAuthError::new(
                "browserOpenFailed",
                format!("Failed to open the system browser: {error}"),
            )
        })
    }
}

trait TokenClient: Send + Sync {
    fn exchange_code(
        &self,
        client_id: &str,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<TokenResponse, TokenRequestError>;

    fn refresh(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<TokenResponse, TokenRequestError>;
}

struct ReqwestTokenClient {
    client: reqwest::blocking::Client,
    token_url: String,
}

impl ReqwestTokenClient {
    fn new(client: reqwest::blocking::Client, token_url: &str) -> Self {
        Self {
            client,
            token_url: token_url.to_owned(),
        }
    }

    fn parse_response(
        response: reqwest::blocking::Response,
    ) -> Result<TokenResponse, TokenRequestError> {
        let status = response.status();
        if status.is_success() {
            return response.json::<TokenResponse>().map_err(|error| {
                TokenRequestError::InvalidResponse(format!(
                    "Spotify returned an invalid token response: {error}"
                ))
            });
        }

        match response.json::<TokenErrorResponse>() {
            Ok(body) => Err(TokenRequestError::OAuth {
                code: body.error,
                description: body.error_description,
            }),
            Err(error) => Err(TokenRequestError::InvalidResponse(format!(
                "Spotify token request failed with HTTP {status}: {error}"
            ))),
        }
    }
}

impl TokenClient for ReqwestTokenClient {
    fn exchange_code(
        &self,
        client_id: &str,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<TokenResponse, TokenRequestError> {
        let response = self
            .client
            .post(&self.token_url)
            .form(&[
                ("client_id", client_id),
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", verifier),
            ])
            .send()
            .map_err(|error| TokenRequestError::Transport(error.to_string()))?;

        Self::parse_response(response)
    }

    fn refresh(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<TokenResponse, TokenRequestError> {
        let response = self
            .client
            .post(&self.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
            ])
            .send()
            .map_err(|error| TokenRequestError::Transport(error.to_string()))?;

        Self::parse_response(response)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

#[derive(Debug, Clone)]
enum TokenRequestError {
    Transport(String),
    OAuth {
        code: String,
        description: Option<String>,
    },
    InvalidResponse(String),
}

struct LoopbackCallback {
    listener: TcpListener,
    redirect_uri: String,
    timeout: Duration,
}

impl LoopbackCallback {
    fn bind(timeout: Duration) -> Result<Self, SpotifyAuthError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(callback_listener_error)?;
        listener
            .set_nonblocking(true)
            .map_err(callback_listener_error)?;
        let port = listener
            .local_addr()
            .map_err(callback_listener_error)?
            .port();

        Ok(Self {
            listener,
            redirect_uri: format!("http://127.0.0.1:{port}{CALLBACK_PATH}"),
            timeout,
        })
    }

    fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    fn receive(&self, expected_state: &str) -> Result<String, SpotifyAuthError> {
        let deadline = Instant::now() + self.timeout;

        loop {
            match self.listener.accept() {
                Ok((mut stream, _)) => return handle_callback(&mut stream, expected_state),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(SpotifyAuthError::new(
                            "callbackTimeout",
                            "Spotify authorization timed out. Try connecting again.",
                        ));
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(callback_listener_error(error)),
            }
        }
    }
}

fn handle_callback(
    stream: &mut TcpStream,
    expected_state: &str,
) -> Result<String, SpotifyAuthError> {
    let mut request_line = String::new();
    BufReader::new(stream.try_clone().map_err(callback_listener_error)?)
        .read_line(&mut request_line)
        .map_err(callback_listener_error)?;

    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| SpotifyAuthError::new("invalidCallback", "Invalid Spotify callback."))?;
    let callback_url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| SpotifyAuthError::new("invalidCallback", "Invalid Spotify callback URL."))?;

    if callback_url.path() != CALLBACK_PATH {
        write_callback_response(
            stream,
            "404 Not Found",
            "Refrain did not request this path.",
        )?;
        return Err(SpotifyAuthError::new(
            "invalidCallback",
            "Spotify returned to an unexpected callback path.",
        ));
    }

    let state = callback_url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()));
    if state.as_deref() != Some(expected_state) {
        write_callback_response(
            stream,
            "400 Bad Request",
            "Authorization state did not match.",
        )?;
        return Err(SpotifyAuthError::new(
            "stateMismatch",
            "Spotify authorization state did not match. Try connecting again.",
        ));
    }

    if let Some(error) = callback_url
        .query_pairs()
        .find_map(|(key, value)| (key == "error").then(|| value.into_owned()))
    {
        write_callback_response(stream, "200 OK", "Spotify authorization was cancelled.")?;
        if error == "access_denied" {
            return Err(SpotifyAuthError::new(
                "authorizationCancelled",
                "Spotify authorization was cancelled.",
            ));
        }
        return Err(SpotifyAuthError::new(
            "authorizationFailed",
            format!("Spotify authorization failed: {error}"),
        ));
    }

    let code = callback_url
        .query_pairs()
        .find_map(|(key, value)| (key == "code").then(|| value.into_owned()))
        .ok_or_else(|| {
            SpotifyAuthError::new(
                "invalidCallback",
                "Spotify callback did not include an authorization code.",
            )
        })?;

    write_callback_response(
        stream,
        "200 OK",
        "Spotify is connected. You can return to Refrain.",
    )?;
    Ok(code)
}

fn write_callback_response(
    stream: &mut TcpStream,
    status: &str,
    message: &str,
) -> Result<(), SpotifyAuthError> {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Refrain</title></head><body><p>{message}</p></body></html>"
    );
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(callback_listener_error)
}

fn build_authorization_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> Result<Url, SpotifyAuthError> {
    let mut url = Url::parse(AUTHORIZE_URL).map_err(|error| {
        SpotifyAuthError::new(
            "authorizationUrlFailed",
            format!("Failed to build Spotify authorization URL: {error}"),
        )
    })?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("scope", SPOTIFY_SCOPES)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge);
    Ok(url)
}

fn random_urlsafe(byte_count: usize) -> Result<String, SpotifyAuthError> {
    let mut bytes = vec![0_u8; byte_count];
    getrandom::fill(&mut bytes).map_err(|error| {
        SpotifyAuthError::new(
            "randomnessUnavailable",
            format!("Failed to generate secure OAuth state: {error}"),
        )
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn map_token_error(error: TokenRequestError, refreshing: bool) -> SpotifyAuthError {
    match error {
        TokenRequestError::OAuth { code, description } if code == "invalid_client" => {
            SpotifyAuthError::new(
                "invalidClientId",
                description.unwrap_or_else(|| "Spotify rejected this Client ID.".into()),
            )
        }
        TokenRequestError::OAuth { code, description } if refreshing && code == "invalid_grant" => {
            SpotifyAuthError::new(
                "reauthorizationRequired",
                description.unwrap_or_else(|| {
                    "Spotify authorization expired or was revoked. Connect Spotify again.".into()
                }),
            )
        }
        TokenRequestError::OAuth { code, description } => SpotifyAuthError::new(
            "authorizationFailed",
            description.unwrap_or_else(|| format!("Spotify authorization failed: {code}")),
        ),
        TokenRequestError::Transport(message) | TokenRequestError::InvalidResponse(message) => {
            SpotifyAuthError::new("tokenRequestFailed", message)
        }
    }
}

fn credential_error(error: CredentialStoreError) -> SpotifyAuthError {
    tracing::error!(%error, "Spotify credential store operation failed");
    SpotifyAuthError::new(
        "credentialStoreUnavailable",
        "The operating system credential store is unavailable. Spotify credentials were not saved.",
    )
}

fn callback_listener_error(error: std::io::Error) -> SpotifyAuthError {
    SpotifyAuthError::new(
        "callbackListenerFailed",
        format!("Spotify loopback callback failed: {error}"),
    )
}

fn lock_error() -> SpotifyAuthError {
    SpotifyAuthError::new(
        "authenticationStateUnavailable",
        "Spotify authentication state is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        net::TcpStream,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ExchangeCall {
        client_id: String,
        code: String,
        redirect_uri: String,
        verifier: String,
    }

    struct MockTokenClient {
        exchange_result: Mutex<Result<TokenResponse, TokenRequestError>>,
        refresh_result: Mutex<Result<TokenResponse, TokenRequestError>>,
        exchange_calls: Mutex<Vec<ExchangeCall>>,
        refresh_calls: AtomicUsize,
    }

    impl MockTokenClient {
        fn success() -> Self {
            Self {
                exchange_result: Mutex::new(Ok(TokenResponse {
                    access_token: "access-one".into(),
                    expires_in: 3600,
                    refresh_token: Some("refresh-one".into()),
                })),
                refresh_result: Mutex::new(Ok(TokenResponse {
                    access_token: "access-two".into(),
                    expires_in: 3600,
                    refresh_token: Some("refresh-two".into()),
                })),
                exchange_calls: Mutex::new(Vec::new()),
                refresh_calls: AtomicUsize::new(0),
            }
        }
    }

    impl TokenClient for MockTokenClient {
        fn exchange_code(
            &self,
            client_id: &str,
            code: &str,
            redirect_uri: &str,
            verifier: &str,
        ) -> Result<TokenResponse, TokenRequestError> {
            self.exchange_calls
                .lock()
                .expect("exchange calls should lock")
                .push(ExchangeCall {
                    client_id: client_id.into(),
                    code: code.into(),
                    redirect_uri: redirect_uri.into(),
                    verifier: verifier.into(),
                });
            self.exchange_result
                .lock()
                .expect("exchange result should lock")
                .clone()
        }

        fn refresh(
            &self,
            _client_id: &str,
            _refresh_token: &str,
        ) -> Result<TokenResponse, TokenRequestError> {
            self.refresh_calls.fetch_add(1, Ordering::Relaxed);
            self.refresh_result
                .lock()
                .expect("refresh result should lock")
                .clone()
        }
    }

    #[derive(Default)]
    struct MockCredentialStore {
        token: Mutex<Option<String>>,
        fail_get: AtomicBool,
        fail_set: AtomicBool,
        fail_clear: AtomicBool,
    }

    impl MockCredentialStore {
        fn with_token(token: &str) -> Self {
            Self {
                token: Mutex::new(Some(token.into())),
                ..Self::default()
            }
        }
    }

    impl RefreshTokenStore for MockCredentialStore {
        fn get_refresh_token(&self) -> Result<Option<String>, CredentialStoreError> {
            if self.fail_get.load(Ordering::Relaxed) {
                return Err(CredentialStoreError::new("get failed"));
            }
            Ok(self.token.lock().expect("token should lock").clone())
        }

        fn set_refresh_token(&self, token: &str) -> Result<(), CredentialStoreError> {
            if self.fail_set.load(Ordering::Relaxed) {
                return Err(CredentialStoreError::new("set failed"));
            }
            self.token
                .lock()
                .expect("token should lock")
                .replace(token.into());
            Ok(())
        }

        fn clear_refresh_token(&self) -> Result<(), CredentialStoreError> {
            if self.fail_clear.load(Ordering::Relaxed) {
                return Err(CredentialStoreError::new("clear failed"));
            }
            self.token.lock().expect("token should lock").take();
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum CallbackMode {
        Success,
        StateMismatch,
        Cancelled,
    }

    struct MockBrowser {
        modes: Mutex<VecDeque<CallbackMode>>,
        opened_urls: Mutex<Vec<String>>,
    }

    impl MockBrowser {
        fn new(modes: impl IntoIterator<Item = CallbackMode>) -> Self {
            Self {
                modes: Mutex::new(modes.into_iter().collect()),
                opened_urls: Mutex::new(Vec::new()),
            }
        }
    }

    impl BrowserOpener for MockBrowser {
        fn open(&self, url: &Url) -> Result<(), SpotifyAuthError> {
            self.opened_urls
                .lock()
                .expect("opened URLs should lock")
                .push(url.to_string());
            let mode = self
                .modes
                .lock()
                .expect("callback modes should lock")
                .pop_front()
                .unwrap_or(CallbackMode::Success);
            let url = url.clone();
            thread::spawn(move || send_test_callback(url, mode));
            Ok(())
        }
    }

    fn send_test_callback(authorization_url: Url, mode: CallbackMode) {
        thread::sleep(Duration::from_millis(20));
        let params = authorization_url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<std::collections::HashMap<_, _>>();
        let redirect_uri = Url::parse(
            params
                .get("redirect_uri")
                .expect("redirect URI should be present"),
        )
        .expect("redirect URI should parse");
        let state = params.get("state").expect("state should be present");
        let query = match mode {
            CallbackMode::Success => format!("code=test-code&state={state}"),
            CallbackMode::StateMismatch => "code=test-code&state=wrong-state".into(),
            CallbackMode::Cancelled => format!("error=access_denied&state={state}"),
        };
        let mut stream = TcpStream::connect((
            redirect_uri.host_str().expect("redirect host should exist"),
            redirect_uri.port().expect("redirect port should exist"),
        ))
        .expect("callback should connect");
        write!(
            stream,
            "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            redirect_uri.path(),
            query
        )
        .expect("callback request should write");
    }

    fn test_client(
        tokens: Arc<MockTokenClient>,
        credentials: Arc<MockCredentialStore>,
        browser: Arc<MockBrowser>,
    ) -> SpotifyClient {
        SpotifyClient::with_dependencies(tokens, credentials, browser, Duration::from_secs(1))
    }

    #[test]
    fn pkce_authorization_uses_matching_verifier_and_dynamic_loopback_port() {
        let tokens = Arc::new(MockTokenClient::success());
        let credentials = Arc::new(MockCredentialStore::default());
        let browser = Arc::new(MockBrowser::new([CallbackMode::Success]));
        let client = test_client(tokens.clone(), credentials.clone(), browser.clone());

        client.connect("client-id").expect("connect should succeed");

        let calls = tokens
            .exchange_calls
            .lock()
            .expect("exchange calls should lock");
        let call = calls.first().expect("exchange should be called");
        let opened_url = browser
            .opened_urls
            .lock()
            .expect("opened URLs should lock")
            .first()
            .cloned()
            .expect("browser should open");
        let authorization_url = Url::parse(&opened_url).expect("authorization URL should parse");
        let params = authorization_url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<std::collections::HashMap<_, _>>();
        let expected_challenge = pkce_challenge(&call.verifier);

        assert_eq!(call.code, "test-code");
        assert_eq!(call.client_id, "client-id");
        assert!(call.redirect_uri.starts_with("http://127.0.0.1:"));
        assert!(call.redirect_uri.ends_with(CALLBACK_PATH));
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            params.get("code_challenge").map(String::as_str),
            Some(expected_challenge.as_str())
        );
        assert_eq!(
            credentials
                .get_refresh_token()
                .expect("credential read should succeed")
                .as_deref(),
            Some("refresh-one")
        );
    }

    #[test]
    fn state_mismatch_stops_before_token_exchange() {
        let tokens = Arc::new(MockTokenClient::success());
        let credentials = Arc::new(MockCredentialStore::default());
        let browser = Arc::new(MockBrowser::new([CallbackMode::StateMismatch]));
        let client = test_client(tokens.clone(), credentials, browser);

        let error = client
            .connect("client-id")
            .expect_err("connect should fail");

        assert_eq!(error.code, "stateMismatch");
        assert!(
            tokens
                .exchange_calls
                .lock()
                .expect("exchange calls should lock")
                .is_empty()
        );
    }

    #[test]
    fn cancelled_authorization_is_reported() {
        let client = test_client(
            Arc::new(MockTokenClient::success()),
            Arc::new(MockCredentialStore::default()),
            Arc::new(MockBrowser::new([CallbackMode::Cancelled])),
        );

        let error = client
            .connect("client-id")
            .expect_err("connect should fail");

        assert_eq!(error.code, "authorizationCancelled");
    }

    #[test]
    fn invalid_client_id_from_spotify_is_structured() {
        let tokens = Arc::new(MockTokenClient::success());
        *tokens
            .exchange_result
            .lock()
            .expect("exchange result should lock") = Err(TokenRequestError::OAuth {
            code: "invalid_client".into(),
            description: Some("Invalid client".into()),
        });
        let client = test_client(
            tokens,
            Arc::new(MockCredentialStore::default()),
            Arc::new(MockBrowser::new([CallbackMode::Success])),
        );

        let error = client
            .connect("client-id")
            .expect_err("connect should fail");

        assert_eq!(error.code, "invalidClientId");
    }

    #[test]
    fn credential_store_failure_does_not_cache_access_token() {
        let credentials = Arc::new(MockCredentialStore::default());
        credentials.fail_set.store(true, Ordering::Relaxed);
        let client = test_client(
            Arc::new(MockTokenClient::success()),
            credentials,
            Arc::new(MockBrowser::new([CallbackMode::Success])),
        );

        let error = client
            .connect("client-id")
            .expect_err("connect should fail");

        assert_eq!(error.code, "credentialStoreUnavailable");
        assert!(
            client
                .access_token
                .lock()
                .expect("access token should lock")
                .is_none()
        );
    }

    #[test]
    fn refresh_uses_secure_credential_and_reuses_cached_access_token() {
        let tokens = Arc::new(MockTokenClient::success());
        let credentials = Arc::new(MockCredentialStore::with_token("refresh-old"));
        let client = test_client(
            tokens.clone(),
            credentials.clone(),
            Arc::new(MockBrowser::new([])),
        );

        assert_eq!(
            client
                .access_token("client-id")
                .expect("refresh should work"),
            "access-two"
        );
        assert_eq!(
            client.access_token("client-id").expect("cache should work"),
            "access-two"
        );
        assert_eq!(tokens.refresh_calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            credentials
                .get_refresh_token()
                .expect("credential read should succeed")
                .as_deref(),
            Some("refresh-two")
        );
    }

    #[test]
    fn expired_refresh_requires_reauthorization_and_clears_credential() {
        let tokens = Arc::new(MockTokenClient::success());
        *tokens
            .refresh_result
            .lock()
            .expect("refresh result should lock") = Err(TokenRequestError::OAuth {
            code: "invalid_grant".into(),
            description: None,
        });
        let credentials = Arc::new(MockCredentialStore::with_token("expired"));
        let client = test_client(tokens, credentials.clone(), Arc::new(MockBrowser::new([])));

        let error = client
            .access_token("client-id")
            .expect_err("refresh should fail");

        assert_eq!(error.code, "reauthorizationRequired");
        assert_eq!(
            credentials
                .get_refresh_token()
                .expect("credential read should succeed"),
            None
        );
    }

    #[test]
    fn disconnect_and_reconnect_replace_authorization_state() {
        let credentials = Arc::new(MockCredentialStore::default());
        let client = test_client(
            Arc::new(MockTokenClient::success()),
            credentials.clone(),
            Arc::new(MockBrowser::new([
                CallbackMode::Success,
                CallbackMode::Success,
            ])),
        );

        client
            .connect("client-id")
            .expect("first connect should work");
        assert!(
            client
                .status(Some("client-id".into()))
                .expect("status should work")
                .connected
        );

        client.disconnect().expect("disconnect should work");
        assert!(
            !client
                .status(Some("client-id".into()))
                .expect("status should work")
                .connected
        );

        client.connect("client-id").expect("reconnect should work");
        assert!(
            client
                .status(Some("client-id".into()))
                .expect("status should work")
                .connected
        );
        assert_eq!(
            credentials
                .get_refresh_token()
                .expect("credential read should succeed")
                .as_deref(),
            Some("refresh-one")
        );
    }
}
