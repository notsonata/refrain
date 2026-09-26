use std::{
    collections::HashSet,
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::header::RETRY_AFTER;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    db::{Database, DatabaseError},
    domain::{SourceAccount, SourceCollection, SourceCollectionItem, SourceTrack},
    security::{KeyringRefreshTokenStore, RefreshTokenStore},
    spotify::{SpotifyAuthError, SpotifyClient},
};

pub const SOURCE_REFRESH_PROGRESS_EVENT: &str = "spotify-source-refresh-progress";

const SPOTIFY_API_BASE: &str = "https://api.spotify.com/v1";
const SPOTIFY_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const LIKED_SONGS_PROVIDER_ID: &str = "spotify:liked-songs";
const PAGE_LIMIT: usize = 50;
const MAX_REQUEST_ATTEMPTS: usize = 3;
const MAX_RATE_LIMIT_WAIT_SECONDS: u64 = 60;

static SPOTIFY_RATE_LIMIT_UNTIL_EPOCH_SECONDS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default)]
pub struct SourceRefreshControl {
    running: AtomicBool,
    cancelled: AtomicBool,
}

impl SourceRefreshControl {
    pub fn begin(self: &Arc<Self>) -> Result<SourceRefreshGuard, SourceRefreshError> {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                SourceRefreshError::new(
                    "refreshAlreadyRunning",
                    "A Spotify source refresh is already running.",
                )
            })?;
        self.cancelled.store(false, Ordering::Release);
        Ok(SourceRefreshGuard {
            control: Arc::clone(self),
        })
    }

    pub fn cancel(&self) -> bool {
        if !self.running.load(Ordering::Acquire) {
            return false;
        }
        self.cancelled.store(true, Ordering::Release);
        true
    }

    fn check_cancelled(&self) -> Result<(), SourceRefreshError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(SourceRefreshError::new(
                "refreshCancelled",
                "Spotify source refresh was cancelled.",
            ));
        }
        Ok(())
    }
}

pub struct SourceRefreshGuard {
    control: Arc<SourceRefreshControl>,
}

impl Drop for SourceRefreshGuard {
    fn drop(&mut self) {
        self.control.cancelled.store(false, Ordering::Release);
        self.control.running.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRefreshProgress {
    pub phase: String,
    pub completed: usize,
    pub total: Option<usize>,
    pub message: String,
}

impl SourceRefreshProgress {
    fn new(
        phase: &str,
        completed: usize,
        total: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: phase.into(),
            completed,
            total,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRefreshSummary {
    pub account_display_name: Option<String>,
    pub liked_songs: usize,
    pub playlists: usize,
    pub refreshed_playlists: usize,
    pub unchanged_playlists: usize,
    pub inaccessible_playlists: usize,
    pub removed_playlists: usize,
    pub synced_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRefreshError {
    pub code: String,
    pub message: String,
}

impl SourceRefreshError {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for SourceRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SourceRefreshError {}

fn unix_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn register_spotify_rate_limit(retry_after_seconds: u64) {
    if retry_after_seconds == 0 {
        return;
    }
    let deadline = unix_epoch_seconds().saturating_add(retry_after_seconds);
    SPOTIFY_RATE_LIMIT_UNTIL_EPOCH_SECONDS.fetch_max(deadline, Ordering::AcqRel);
}

pub(crate) fn spotify_rate_limit_remaining_seconds() -> u64 {
    SPOTIFY_RATE_LIMIT_UNTIL_EPOCH_SECONDS
        .load(Ordering::Acquire)
        .saturating_sub(unix_epoch_seconds())
}

pub(crate) fn check_spotify_rate_limit() -> Result<(), SourceRefreshError> {
    let remaining = spotify_rate_limit_remaining_seconds();
    if remaining == 0 {
        return Ok(());
    }
    Err(SourceRefreshError::new(
        "spotifyRateLimited",
        spotify_rate_limit_message(remaining),
    ))
}

pub(crate) fn spotify_rate_limit_message(retry_after_seconds: u64) -> String {
    let wait = if retry_after_seconds >= 3_600 {
        let rounded_minutes = (retry_after_seconds + 30) / 60;
        let hours = rounded_minutes / 60;
        let minutes = rounded_minutes % 60;
        if minutes > 0 {
            format!("{hours} hr {minutes} min")
        } else {
            format!("{hours} hr")
        }
    } else {
        let minutes = retry_after_seconds / 60;
        let seconds = retry_after_seconds % 60;
        if minutes > 0 && seconds > 0 {
            format!("{minutes} min {seconds} sec")
        } else if minutes > 0 {
            format!("{minutes} min")
        } else {
            format!("{seconds} sec")
        }
    };

    format!("Spotify rate limit reached. Try again in about {wait}.")
}

pub fn refresh_spotify_source(
    database: &Database,
    spotify: Arc<SpotifyClient>,
    client_id: &str,
    control: &SourceRefreshControl,
    progress: impl Fn(SourceRefreshProgress),
) -> Result<SourceRefreshSummary, SourceRefreshError> {
    let transport = ReqwestTransport::new()?;
    let token_provider = ProductionAccessTokenProvider::new(spotify, client_id)?;
    let mut api = SpotifyApiClient::new(transport, token_provider, SPOTIFY_API_BASE);
    refresh_with_api(database, &mut api, client_id, control, progress)
}

fn refresh_with_api<A: SpotifySourceApi>(
    database: &Database,
    api: &mut A,
    client_id: &str,
    control: &SourceRefreshControl,
    progress: impl Fn(SourceRefreshProgress),
) -> Result<SourceRefreshSummary, SourceRefreshError> {
    control.check_cancelled()?;
    progress(SourceRefreshProgress::new(
        "profile",
        0,
        None,
        "Loading Spotify account",
    ));
    let profile = api.profile(control)?;
    let account_id = database
        .upsert_source_account(&SourceAccount {
            provider: "spotify".into(),
            provider_account_id: profile.provider_account_id.clone(),
            display_name: profile.display_name.clone(),
            image_url: profile.image_url.clone(),
            client_id: client_id.into(),
        })
        .map_err(|error| database_error("save Spotify account", error))?;

    control.check_cancelled()?;
    progress(SourceRefreshProgress::new(
        "likedSongs",
        0,
        None,
        "Refreshing Liked Songs",
    ));
    let liked_songs = api.liked_songs(control)?;
    database
        .replace_source_collection(
            &SourceCollection {
                source_account_id: account_id,
                provider_collection_id: LIKED_SONGS_PROVIDER_ID.into(),
                kind: "liked_songs".into(),
                name: "Liked Songs".into(),
                snapshot_id: None,
                owner_provider_id: Some(profile.provider_account_id.clone()),
                is_accessible: true,
                access_issue: None,
                image_url: None,
                external_url: None,
                album_metadata: None,
            },
            &liked_songs,
        )
        .map_err(|error| database_error("save Liked Songs", error))?;

    control.check_cancelled()?;
    progress(SourceRefreshProgress::new(
        "playlists",
        0,
        None,
        "Loading Spotify playlists",
    ));
    let playlists = api.playlists(control)?;
    let playlist_ids = playlists
        .iter()
        .map(|playlist| playlist.id.clone())
        .collect::<HashSet<_>>();
    let mut refreshed_playlists = 0;
    let mut unchanged_playlists = 0;
    let mut inaccessible_playlists = 0;

    for (index, playlist) in playlists.iter().enumerate() {
        control.check_cancelled()?;
        progress(SourceRefreshProgress::new(
            "playlistItems",
            index,
            Some(playlists.len()),
            format!("Refreshing {}", playlist.name),
        ));

        let stored = database
            .source_collection_state(account_id, &playlist.id)
            .map_err(|error| database_error("load playlist state", error))?;
        let snapshot_unchanged = playlist.snapshot_id.is_some()
            && stored.as_ref().is_some_and(|state| {
                state.is_accessible
                    && state.snapshot_id == playlist.snapshot_id
                    && playlist
                        .item_count
                        .is_some_and(|item_count| item_count == state.entry_count)
            });

        let base_collection = SourceCollection {
            source_account_id: account_id,
            provider_collection_id: playlist.id.clone(),
            kind: "playlist".into(),
            name: playlist.name.clone(),
            snapshot_id: playlist.snapshot_id.clone(),
            owner_provider_id: playlist.owner_provider_id.clone(),
            is_accessible: true,
            access_issue: None,
            image_url: playlist.image_url.clone(),
            external_url: playlist.external_url.clone(),
            album_metadata: None,
        };

        if snapshot_unchanged {
            database
                .upsert_source_collection(&base_collection)
                .map_err(|error| database_error("update playlist metadata", error))?;
            unchanged_playlists += 1;
            continue;
        }

        match api.playlist_items(&playlist.id, control) {
            Ok(items) => {
                database
                    .replace_source_collection(&base_collection, &items)
                    .map_err(|error| database_error("save playlist", error))?;
                refreshed_playlists += 1;
            }
            Err(error) if error.code == "spotifyForbidden" => {
                let mut inaccessible = base_collection;
                inaccessible.is_accessible = false;
                inaccessible.access_issue = Some(
                    "Spotify only allows item access for playlists you own or collaborate on."
                        .into(),
                );
                database.upsert_source_collection(&inaccessible).map_err(
                    |database_error_value| {
                        database_error("save inaccessible playlist", database_error_value)
                    },
                )?;
                inaccessible_playlists += 1;
            }
            Err(error) => return Err(error),
        }
    }

    control.check_cancelled()?;
    let removed_playlists = database
        .retain_source_playlists(account_id, &playlist_ids)
        .map_err(|error| database_error("remove stale Spotify playlists", error))?;
    let synced_at = database
        .mark_source_account_synced_now(account_id)
        .map_err(|error| database_error("record Spotify refresh time", error))?;

    progress(SourceRefreshProgress::new(
        "complete",
        playlists.len(),
        Some(playlists.len()),
        "Spotify source refresh complete",
    ));

    Ok(SourceRefreshSummary {
        account_display_name: profile.display_name,
        liked_songs: liked_songs.len(),
        playlists: playlists.len(),
        refreshed_playlists,
        unchanged_playlists,
        inaccessible_playlists,
        removed_playlists,
        synced_at,
    })
}

fn database_error(operation: &str, error: DatabaseError) -> SourceRefreshError {
    tracing::error!(%error, operation, "Spotify source persistence failed");
    SourceRefreshError::new("persistenceFailed", format!("Failed to {operation}."))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpotifyProfile {
    provider_account_id: String,
    display_name: Option<String>,
    image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpotifyPlaylist {
    id: String,
    name: String,
    snapshot_id: Option<String>,
    owner_provider_id: Option<String>,
    item_count: Option<usize>,
    image_url: Option<String>,
    external_url: Option<String>,
}

trait SpotifySourceApi {
    fn profile(
        &mut self,
        control: &SourceRefreshControl,
    ) -> Result<SpotifyProfile, SourceRefreshError>;

    fn liked_songs(
        &mut self,
        control: &SourceRefreshControl,
    ) -> Result<Vec<SourceCollectionItem>, SourceRefreshError>;

    fn playlists(
        &mut self,
        control: &SourceRefreshControl,
    ) -> Result<Vec<SpotifyPlaylist>, SourceRefreshError>;

    fn playlist_items(
        &mut self,
        playlist_id: &str,
        control: &SourceRefreshControl,
    ) -> Result<Vec<SourceCollectionItem>, SourceRefreshError>;
}

trait HttpTransport {
    fn get(&self, url: &str, bearer_token: &str) -> Result<HttpResponse, SourceRefreshError>;
}

#[derive(Debug, Clone)]
struct HttpResponse {
    status: u16,
    retry_after_seconds: Option<u64>,
    body: String,
}

struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    fn new() -> Result<Self, SourceRefreshError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| {
                SourceRefreshError::new(
                    "httpClientFailed",
                    format!("Failed to initialize Spotify HTTP client: {error}"),
                )
            })?;
        Ok(Self { client })
    }
}

impl HttpTransport for ReqwestTransport {
    fn get(&self, url: &str, bearer_token: &str) -> Result<HttpResponse, SourceRefreshError> {
        let response = self
            .client
            .get(url)
            .bearer_auth(bearer_token)
            .send()
            .map_err(|error| {
                SourceRefreshError::new(
                    "spotifyNetworkFailed",
                    format!("Spotify request failed: {error}"),
                )
            })?;
        let status = response.status().as_u16();
        let retry_after_seconds = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());
        let body = response.text().map_err(|error| {
            SourceRefreshError::new(
                "spotifyResponseFailed",
                format!("Failed to read Spotify response: {error}"),
            )
        })?;
        Ok(HttpResponse {
            status,
            retry_after_seconds,
            body,
        })
    }
}

trait AccessTokenProvider {
    fn access_token(&self) -> Result<String, SourceRefreshError>;
    fn refresh_access_token(&self) -> Result<String, SourceRefreshError>;
}

struct ProductionAccessTokenProvider {
    spotify: Arc<SpotifyClient>,
    client_id: String,
    token_client: reqwest::blocking::Client,
}

impl ProductionAccessTokenProvider {
    fn new(spotify: Arc<SpotifyClient>, client_id: &str) -> Result<Self, SourceRefreshError> {
        let token_client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| {
                SourceRefreshError::new(
                    "httpClientFailed",
                    format!("Failed to initialize Spotify token client: {error}"),
                )
            })?;
        Ok(Self {
            spotify,
            client_id: client_id.into(),
            token_client,
        })
    }
}

impl AccessTokenProvider for ProductionAccessTokenProvider {
    fn access_token(&self) -> Result<String, SourceRefreshError> {
        self.spotify
            .access_token(&self.client_id)
            .map_err(source_auth_error)
    }

    fn refresh_access_token(&self) -> Result<String, SourceRefreshError> {
        let credentials = KeyringRefreshTokenStore;
        let refresh_token = credentials
            .get_refresh_token()
            .map_err(|error| {
                tracing::error!(%error, "Spotify credential read failed during source refresh");
                SourceRefreshError::new(
                    "credentialStoreUnavailable",
                    "The operating system credential store is unavailable.",
                )
            })?
            .ok_or_else(|| {
                SourceRefreshError::new(
                    "notConnected",
                    "Connect Spotify before refreshing source data.",
                )
            })?;

        let response = self
            .token_client
            .post(SPOTIFY_TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", self.client_id.as_str()),
            ])
            .send()
            .map_err(|error| {
                SourceRefreshError::new(
                    "tokenRequestFailed",
                    format!("Spotify token refresh failed: {error}"),
                )
            })?;
        let status = response.status();
        let body = response.text().map_err(|error| {
            SourceRefreshError::new(
                "tokenRequestFailed",
                format!("Failed to read Spotify token response: {error}"),
            )
        })?;

        if !status.is_success() {
            let token_error = serde_json::from_str::<SpotifyTokenErrorDto>(&body).ok();
            if token_error
                .as_ref()
                .is_some_and(|error| error.error == "invalid_grant")
            {
                credentials.clear_refresh_token().map_err(|error| {
                    tracing::error!(%error, "Spotify credential clear failed after invalid grant");
                    SourceRefreshError::new(
                        "credentialStoreUnavailable",
                        "Spotify authorization expired, but the stored credential could not be cleared.",
                    )
                })?;
                return Err(SourceRefreshError::new(
                    "reauthorizationRequired",
                    "Spotify authorization expired or was revoked. Connect Spotify again.",
                ));
            }
            return Err(SourceRefreshError::new(
                "tokenRequestFailed",
                token_error
                    .and_then(|error| error.error_description)
                    .unwrap_or_else(|| format!("Spotify token refresh failed with HTTP {status}.")),
            ));
        }

        let token = serde_json::from_str::<SpotifyTokenDto>(&body).map_err(|error| {
            SourceRefreshError::new(
                "tokenRequestFailed",
                format!("Spotify returned an invalid token response: {error}"),
            )
        })?;
        if let Some(rotated_refresh_token) = token.refresh_token.as_deref() {
            credentials
                .set_refresh_token(rotated_refresh_token)
                .map_err(|error| {
                    tracing::error!(%error, "Spotify rotated credential save failed");
                    SourceRefreshError::new(
                        "credentialStoreUnavailable",
                        "Spotify refreshed authorization, but the new credential could not be saved.",
                    )
                })?;
        }
        Ok(token.access_token)
    }
}

fn source_auth_error(error: SpotifyAuthError) -> SourceRefreshError {
    SourceRefreshError::new(error.code, error.message)
}

#[derive(Debug, Deserialize)]
struct SpotifyTokenDto {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyTokenErrorDto {
    error: String,
    error_description: Option<String>,
}

struct SpotifyApiClient<T, P> {
    transport: T,
    tokens: P,
    api_base: String,
    access_token: Option<String>,
}

impl<T: HttpTransport, P: AccessTokenProvider> SpotifyApiClient<T, P> {
    fn new(transport: T, tokens: P, api_base: &str) -> Self {
        Self {
            transport,
            tokens,
            api_base: api_base.trim_end_matches('/').into(),
            access_token: None,
        }
    }

    fn request_json<D: for<'de> Deserialize<'de>>(
        &mut self,
        url: &str,
        control: &SourceRefreshControl,
    ) -> Result<D, SourceRefreshError> {
        check_spotify_rate_limit()?;
        let mut refreshed_after_unauthorized = false;
        for attempt in 0..MAX_REQUEST_ATTEMPTS {
            control.check_cancelled()?;
            let token = match self.access_token.clone() {
                Some(token) => token,
                None => {
                    let token = self.tokens.access_token()?;
                    self.access_token = Some(token.clone());
                    token
                }
            };

            let response = match self.transport.get(url, &token) {
                Ok(response) => response,
                Err(error)
                    if error.code == "spotifyNetworkFailed"
                        && attempt + 1 < MAX_REQUEST_ATTEMPTS =>
                {
                    sleep_interruptibly(
                        Duration::from_millis(250 * (attempt as u64 + 1)),
                        control,
                    )?;
                    continue;
                }
                Err(error) => return Err(error),
            };

            match response.status {
                200..=299 => {
                    return serde_json::from_str(&response.body).map_err(|error| {
                        SourceRefreshError::new(
                            "spotifyInvalidResponse",
                            format!("Spotify returned an invalid response: {error}"),
                        )
                    });
                }
                401 if !refreshed_after_unauthorized => {
                    let token = self.tokens.refresh_access_token()?;
                    self.access_token = Some(token);
                    refreshed_after_unauthorized = true;
                    continue;
                }
                401 => {
                    return Err(SourceRefreshError::new(
                        "reauthorizationRequired",
                        "Spotify rejected the refreshed access token. Connect Spotify again.",
                    ));
                }
                403 => {
                    return Err(SourceRefreshError::new(
                        "spotifyForbidden",
                        "Spotify does not allow access to this resource.",
                    ));
                }
                429 if attempt + 1 < MAX_REQUEST_ATTEMPTS => {
                    let retry_after = response.retry_after_seconds.unwrap_or(1);
                    register_spotify_rate_limit(retry_after);
                    if retry_after > MAX_RATE_LIMIT_WAIT_SECONDS {
                        return Err(SourceRefreshError::new(
                            "spotifyRateLimited",
                            spotify_rate_limit_message(retry_after),
                        ));
                    }
                    sleep_interruptibly(Duration::from_secs(retry_after), control)?;
                    continue;
                }
                429 => {
                    let retry_after = response.retry_after_seconds.unwrap_or(1);
                    register_spotify_rate_limit(retry_after);
                    return Err(SourceRefreshError::new(
                        "spotifyRateLimited",
                        spotify_rate_limit_message(retry_after),
                    ));
                }
                500..=599 if attempt + 1 < MAX_REQUEST_ATTEMPTS => {
                    sleep_interruptibly(
                        Duration::from_millis(250 * (attempt as u64 + 1)),
                        control,
                    )?;
                    continue;
                }
                status => {
                    return Err(SourceRefreshError::new(
                        "spotifyRequestFailed",
                        format!("Spotify request failed with HTTP {status}."),
                    ));
                }
            }
        }

        Err(SourceRefreshError::new(
            "spotifyRequestFailed",
            "Spotify request failed after retrying.",
        ))
    }
}

impl<T: HttpTransport, P: AccessTokenProvider> SpotifySourceApi for SpotifyApiClient<T, P> {
    fn profile(
        &mut self,
        control: &SourceRefreshControl,
    ) -> Result<SpotifyProfile, SourceRefreshError> {
        let url = format!("{}/me", self.api_base);
        let profile = self.request_json::<SpotifyProfileDto>(&url, control)?;
        let provider_account_id = profile.account_id.or(profile.id).ok_or_else(|| {
            SourceRefreshError::new(
                "spotifyInvalidResponse",
                "Spotify profile did not include an account identifier.",
            )
        })?;
        Ok(SpotifyProfile {
            provider_account_id,
            display_name: profile.display_name,
            image_url: profile.images.into_iter().find_map(|image| image.url),
        })
    }

    fn liked_songs(
        &mut self,
        control: &SourceRefreshControl,
    ) -> Result<Vec<SourceCollectionItem>, SourceRefreshError> {
        let mut next = Some(format!(
            "{}/me/tracks?limit={PAGE_LIMIT}&offset=0",
            self.api_base
        ));
        let mut items = Vec::new();

        while let Some(url) = next {
            control.check_cancelled()?;
            let page = self.request_json::<SpotifyPage<SpotifySavedTrackDto>>(&url, control)?;
            for saved in page.items {
                let position = items.len() as i64;
                let provider_item_uri = saved.track.as_ref().and_then(|track| track.uri.clone());
                let track = saved.track.as_ref().and_then(map_spotify_track);
                items.push(SourceCollectionItem {
                    position,
                    track,
                    provider_item_uri,
                    item_type: "track".into(),
                    added_at: saved.added_at.as_deref().and_then(parse_spotify_timestamp),
                    unavailable_reason: saved
                        .track
                        .as_ref()
                        .and_then(|track| {
                            map_spotify_track(track)
                                .is_none()
                                .then_some("unavailable_track".into())
                        })
                        .or_else(|| {
                            saved
                                .track
                                .is_none()
                                .then_some("removed_or_unavailable".into())
                        }),
                });
            }
            next = page.next;
        }

        Ok(items)
    }

    fn playlists(
        &mut self,
        control: &SourceRefreshControl,
    ) -> Result<Vec<SpotifyPlaylist>, SourceRefreshError> {
        let mut next = Some(format!(
            "{}/me/playlists?limit={PAGE_LIMIT}&offset=0",
            self.api_base
        ));
        let mut playlists = Vec::new();

        while let Some(url) = next {
            control.check_cancelled()?;
            let page = self.request_json::<SpotifyPage<SpotifyPlaylistDto>>(&url, control)?;
            for playlist in page.items {
                let id = playlist.id.ok_or_else(|| {
                    SourceRefreshError::new(
                        "spotifyInvalidResponse",
                        "Spotify returned a playlist without an identifier.",
                    )
                })?;
                let image_url = playlist.images.into_iter().find_map(|image| image.url);
                playlists.push(SpotifyPlaylist {
                    id,
                    name: playlist.name.unwrap_or_else(|| "Untitled playlist".into()),
                    snapshot_id: playlist.snapshot_id,
                    owner_provider_id: playlist
                        .owner
                        .and_then(|owner| owner.account_id.or(owner.id)),
                    item_count: playlist
                        .items
                        .and_then(|items| items.total)
                        .or_else(|| playlist.tracks.and_then(|tracks| tracks.total)),
                    image_url,
                    external_url: playlist.external_urls.and_then(|urls| urls.spotify),
                });
            }
            next = page.next;
        }

        Ok(playlists)
    }

    fn playlist_items(
        &mut self,
        playlist_id: &str,
        control: &SourceRefreshControl,
    ) -> Result<Vec<SourceCollectionItem>, SourceRefreshError> {
        let mut next = Some(format!(
            "{}/playlists/{playlist_id}/items?limit={PAGE_LIMIT}&offset=0&additional_types=track,episode",
            self.api_base
        ));
        let mut items = Vec::new();

        while let Some(url) = next {
            control.check_cancelled()?;
            let page = self.request_json::<SpotifyPage<SpotifyPlaylistItemDto>>(&url, control)?;
            for playlist_item in page.items {
                let position = items.len() as i64;
                items.push(map_playlist_item(playlist_item, position));
            }
            next = page.next;
        }

        Ok(items)
    }
}

fn sleep_interruptibly(
    duration: Duration,
    control: &SourceRefreshControl,
) -> Result<(), SourceRefreshError> {
    let mut remaining = duration;
    let slice = Duration::from_millis(100);
    while !remaining.is_zero() {
        control.check_cancelled()?;
        let current = remaining.min(slice);
        thread::sleep(current);
        remaining = remaining.saturating_sub(current);
    }
    control.check_cancelled()
}

#[derive(Debug, Deserialize)]
struct SpotifyProfileDto {
    account_id: Option<String>,
    id: Option<String>,
    display_name: Option<String>,
    #[serde(default)]
    images: Vec<SpotifyImageDto>,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct SpotifyPage<T> {
    #[serde(default)]
    items: Vec<T>,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifySavedTrackDto {
    added_at: Option<String>,
    track: Option<SpotifyTrackDto>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistDto {
    id: Option<String>,
    name: Option<String>,
    snapshot_id: Option<String>,
    owner: Option<SpotifyOwnerDto>,
    items: Option<SpotifyCollectionCountDto>,
    tracks: Option<SpotifyCollectionCountDto>,
    #[serde(default)]
    images: Vec<SpotifyImageDto>,
    external_urls: Option<SpotifyExternalUrlsDto>,
}

#[derive(Debug, Deserialize)]
struct SpotifyOwnerDto {
    account_id: Option<String>,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyCollectionCountDto {
    total: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistItemDto {
    added_at: Option<String>,
    item: Option<Value>,
    track: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SpotifyTrackDto {
    id: Option<String>,
    uri: Option<String>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<SpotifyArtistDto>,
    album: Option<SpotifyAlbumDto>,
    duration_ms: Option<i64>,
    disc_number: Option<i64>,
    track_number: Option<i64>,
    explicit: Option<bool>,
    external_ids: Option<SpotifyExternalIdsDto>,
    external_urls: Option<SpotifyExternalUrlsDto>,
    is_local: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtistDto {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyAlbumDto {
    name: Option<String>,
    release_date: Option<String>,
    #[serde(default)]
    images: Vec<SpotifyImageDto>,
}

#[derive(Debug, Deserialize)]
struct SpotifyImageDto {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyExternalIdsDto {
    isrc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyExternalUrlsDto {
    spotify: Option<String>,
}

fn map_playlist_item(item: SpotifyPlaylistItemDto, position: i64) -> SourceCollectionItem {
    let added_at = item.added_at.as_deref().and_then(parse_spotify_timestamp);
    let playable = item.item.or(item.track);
    let Some(playable) = playable else {
        return SourceCollectionItem {
            position,
            track: None,
            provider_item_uri: None,
            item_type: "track".into(),
            added_at,
            unavailable_reason: Some("removed_or_unavailable".into()),
        };
    };

    let item_type = playable
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let provider_item_uri = playable
        .get("uri")
        .and_then(Value::as_str)
        .map(str::to_owned);

    if item_type == "track" {
        let track = serde_json::from_value::<SpotifyTrackDto>(playable)
            .ok()
            .and_then(|track| map_spotify_track(&track));
        let unavailable_reason = track.is_none().then_some("unavailable_track".into());
        return SourceCollectionItem {
            position,
            track,
            provider_item_uri,
            item_type,
            added_at,
            unavailable_reason,
        };
    }

    let unavailable_reason = if item_type == "episode" {
        "unsupported_episode"
    } else {
        "unsupported_item"
    };
    SourceCollectionItem {
        position,
        track: None,
        provider_item_uri,
        item_type,
        added_at,
        unavailable_reason: Some(unavailable_reason.into()),
    }
}

fn map_spotify_track(track: &SpotifyTrackDto) -> Option<SourceTrack> {
    if track.is_local.unwrap_or(false) {
        return None;
    }
    let provider_track_id = track.id.clone()?;
    let title = track.name.clone()?;
    let artists = track
        .artists
        .iter()
        .filter_map(|artist| artist.name.clone())
        .collect::<Vec<_>>();
    let artists_json = serde_json::to_string(&artists).ok()?;
    let normalized_artists = artists
        .iter()
        .map(|artist| normalize_source_text(artist))
        .collect::<Vec<_>>()
        .join(" | ");
    let album = track.album.as_ref().and_then(|album| album.name.clone());

    Some(SourceTrack {
        provider: "spotify".into(),
        provider_track_id,
        uri: track.uri.clone(),
        isrc: track
            .external_ids
            .as_ref()
            .and_then(|external_ids| external_ids.isrc.clone()),
        normalized_title: normalize_source_text(&title),
        title,
        artists_json,
        normalized_artists,
        normalized_album: album.as_deref().map(normalize_source_text),
        album,
        duration_ms: track.duration_ms,
        disc_number: track.disc_number,
        track_number: track.track_number,
        release_year: track
            .album
            .as_ref()
            .and_then(|album| album.release_date.as_deref())
            .and_then(parse_release_year),
        explicit: track.explicit,
        version_kind: None,
        version_detail: None,
        image_url: track
            .album
            .as_ref()
            .and_then(|album| album.images.iter().find_map(|image| image.url.clone())),
        external_url: track
            .external_urls
            .as_ref()
            .and_then(|urls| urls.spotify.clone()),
    })
}

fn normalize_source_text(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_release_year(value: &str) -> Option<i64> {
    value.get(0..4)?.parse().ok()
}

fn parse_spotify_timestamp(value: &str) -> Option<i64> {
    if value.len() < 20 || !value.ends_with('Z') {
        return None;
    }
    let year = value.get(0..4)?.parse::<i32>().ok()?;
    let month = value.get(5..7)?.parse::<u32>().ok()?;
    let day = value.get(8..10)?.parse::<u32>().ok()?;
    let hour = value.get(11..13)?.parse::<u32>().ok()?;
    let minute = value.get(14..16)?.parse::<u32>().ok()?;
    let second = value.get(17..19)?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let fraction = value
        .get(19..value.len() - 1)?
        .strip_prefix('.')
        .unwrap_or("");
    let milliseconds = if fraction.is_empty() {
        0
    } else {
        let digits = fraction.chars().take(3).collect::<String>();
        let padded = format!("{digits:0<3}");
        padded.parse::<i64>().ok()?
    };
    let days = days_from_civil(year, month, day);
    Some(
        (days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))
            * 1_000
            + milliseconds,
    )
}

fn days_from_civil(mut year: i32, month: u32, day: u32) -> i64 {
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        fs,
        path::PathBuf,
        sync::{
            Mutex,
            atomic::{AtomicU64, AtomicUsize, Ordering},
        },
    };

    use super::*;

    #[test]
    fn rate_limit_message_formats_long_retry_after_readably() {
        assert_eq!(
            spotify_rate_limit_message(10_246),
            "Spotify rate limit reached. Try again in about 2 hr 51 min."
        );
        assert_eq!(
            spotify_rate_limit_message(75),
            "Spotify rate limit reached. Try again in about 1 min 15 sec."
        );
    }

    #[derive(Default)]
    struct MockTransport {
        responses: Mutex<VecDeque<Result<HttpResponse, SourceRefreshError>>>,
        calls: Mutex<Vec<(String, String)>>,
    }

    impl MockTransport {
        fn with_responses(
            responses: impl IntoIterator<Item = Result<HttpResponse, SourceRefreshError>>,
        ) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl HttpTransport for MockTransport {
        fn get(&self, url: &str, bearer_token: &str) -> Result<HttpResponse, SourceRefreshError> {
            self.calls
                .lock()
                .expect("calls should lock")
                .push((url.into(), bearer_token.into()));
            self.responses
                .lock()
                .expect("responses should lock")
                .pop_front()
                .expect("mock response should exist")
        }
    }

    struct MockTokens {
        initial: String,
        refreshed: String,
        refresh_count: AtomicUsize,
    }

    impl MockTokens {
        fn new() -> Self {
            Self {
                initial: "initial-token".into(),
                refreshed: "refreshed-token".into(),
                refresh_count: AtomicUsize::new(0),
            }
        }
    }

    impl AccessTokenProvider for MockTokens {
        fn access_token(&self) -> Result<String, SourceRefreshError> {
            Ok(self.initial.clone())
        }

        fn refresh_access_token(&self) -> Result<String, SourceRefreshError> {
            self.refresh_count.fetch_add(1, Ordering::Relaxed);
            Ok(self.refreshed.clone())
        }
    }

    fn response(status: u16, body: &str) -> Result<HttpResponse, SourceRefreshError> {
        Ok(HttpResponse {
            status,
            retry_after_seconds: None,
            body: body.into(),
        })
    }

    #[test]
    fn liked_songs_pagination_preserves_order() {
        let transport = MockTransport::with_responses([
            response(
                200,
                r#"{"items":[{"added_at":"2026-09-01T00:00:00Z","track":{"id":"one","uri":"spotify:track:one","name":"One","artists":[{"name":"Artist"}]}}],"next":"https://api.test/page-2"}"#,
            ),
            response(
                200,
                r#"{"items":[{"added_at":"2026-09-02T00:00:00Z","track":{"id":"two","uri":"spotify:track:two","name":"Two","artists":[{"name":"Artist"}]}}],"next":null}"#,
            ),
        ]);
        let tokens = MockTokens::new();
        let mut client = SpotifyApiClient::new(transport, tokens, "https://api.test");

        let items = client
            .liked_songs(&SourceRefreshControl::default())
            .expect("liked songs should load");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].position, 0);
        assert_eq!(items[1].position, 1);
        assert_eq!(
            items[1]
                .track
                .as_ref()
                .map(|track| track.provider_track_id.as_str()),
            Some("two")
        );
    }

    #[test]
    fn playlists_use_list_response_artwork_without_extra_cover_request() {
        let transport = MockTransport::with_responses([response(
            200,
            r#"{"items":[{"id":"playlist-one","name":"Playlist One","snapshot_id":"snap","owner":{"id":"account"},"tracks":{"total":3},"images":[{"url":"https://i.scdn.co/image/custom-cover"}],"external_urls":{"spotify":"https://open.spotify.com/playlist/playlist-one"}}],"next":null}"#,
        )]);
        let tokens = MockTokens::new();
        let mut client = SpotifyApiClient::new(transport, tokens, "https://api.test");

        let playlists = client
            .playlists(&SourceRefreshControl::default())
            .expect("playlists should load");

        assert_eq!(playlists.len(), 1);
        assert_eq!(
            playlists[0].image_url.as_deref(),
            Some("https://i.scdn.co/image/custom-cover")
        );
        let calls = client.transport.calls.lock().expect("calls should lock");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains("/me/playlists"));
    }

    #[test]
    fn rate_limit_retries_after_retry_after_delay() {
        let transport = MockTransport::with_responses([
            Ok(HttpResponse {
                status: 429,
                retry_after_seconds: Some(0),
                body: String::new(),
            }),
            response(200, r#"{"account_id":"account","display_name":"Listener"}"#),
        ]);
        let tokens = MockTokens::new();
        let mut client = SpotifyApiClient::new(transport, tokens, "https://api.test");

        let profile = client
            .profile(&SourceRefreshControl::default())
            .expect("profile should retry");

        assert_eq!(profile.provider_account_id, "account");
        assert_eq!(
            client
                .transport
                .calls
                .lock()
                .expect("calls should lock")
                .len(),
            2
        );
    }

    #[test]
    fn unauthorized_request_refreshes_token_once() {
        let transport = MockTransport::with_responses([
            response(401, "{}"),
            response(200, r#"{"account_id":"account","display_name":"Listener"}"#),
        ]);
        let tokens = MockTokens::new();
        let mut client = SpotifyApiClient::new(transport, tokens, "https://api.test");

        client
            .profile(&SourceRefreshControl::default())
            .expect("profile should refresh token");

        assert_eq!(client.tokens.refresh_count.load(Ordering::Relaxed), 1);
        let calls = client.transport.calls.lock().expect("calls should lock");
        assert_eq!(calls[0].1, "initial-token");
        assert_eq!(calls[1].1, "refreshed-token");
    }

    #[test]
    fn playlist_items_support_current_item_field_and_unavailable_entries() {
        let transport = MockTransport::with_responses([response(
            200,
            r#"{"items":[{"added_at":"2026-09-01T01:02:03.456Z","item":{"type":"track","id":"one","uri":"spotify:track:one","name":"One","artists":[{"name":"Artist"}]}},{"added_at":null,"item":null},{"added_at":null,"item":{"type":"episode","uri":"spotify:episode:one"}}],"next":null}"#,
        )]);
        let tokens = MockTokens::new();
        let mut client = SpotifyApiClient::new(transport, tokens, "https://api.test");

        let items = client
            .playlist_items("playlist", &SourceRefreshControl::default())
            .expect("playlist items should load");

        assert_eq!(items.len(), 3);
        assert!(items[0].track.is_some());
        assert!(items[0].added_at.is_some());
        assert_eq!(
            items[1].unavailable_reason.as_deref(),
            Some("removed_or_unavailable")
        );
        assert_eq!(items[2].item_type, "episode");
        assert_eq!(
            items[2].unavailable_reason.as_deref(),
            Some("unsupported_episode")
        );
    }

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath {
        path: PathBuf,
    }

    impl TestDatabasePath {
        fn new(name: &str) -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
            Self {
                path: std::env::temp_dir().join(format!(
                    "refrain-source-sync-{name}-{}-{id}.sqlite3",
                    std::process::id()
                )),
            }
        }
    }

    impl Drop for TestDatabasePath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.path.with_extension("sqlite3-shm"));
        }
    }

    fn sample_track(id: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: id.into(),
            uri: Some(format!("spotify:track:{id}")),
            isrc: None,
            title: "Track".into(),
            normalized_title: "track".into(),
            artists_json: "[\"Artist\"]".into(),
            normalized_artists: "artist".into(),
            album: None,
            normalized_album: None,
            duration_ms: None,
            disc_number: None,
            track_number: None,
            release_year: None,
            explicit: None,
            version_kind: None,
            version_detail: None,
            image_url: None,
            external_url: None,
        }
    }

    fn sample_item(id: &str) -> SourceCollectionItem {
        SourceCollectionItem {
            position: 0,
            track: Some(sample_track(id)),
            provider_item_uri: Some(format!("spotify:track:{id}")),
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }
    }

    fn setup_existing_playlist(name: &str, snapshot: &str) -> (TestDatabasePath, Database, i64) {
        let path = TestDatabasePath::new(name);
        let database = Database::open(path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "account".into(),
                display_name: Some("Listener".into()),
                image_url: None,
                client_id: "client".into(),
            })
            .expect("account should save");
        database
            .replace_source_collection(
                &SourceCollection {
                    source_account_id: account_id,
                    provider_collection_id: "playlist".into(),
                    kind: "playlist".into(),
                    name: "Playlist".into(),
                    snapshot_id: Some(snapshot.into()),
                    owner_provider_id: Some("account".into()),
                    is_accessible: true,
                    access_issue: None,
                    image_url: None,
                    external_url: None,
                    album_metadata: None,
                },
                &[sample_item("old")],
            )
            .expect("playlist should save");
        (path, database, account_id)
    }

    struct FakeSourceApi {
        playlist: SpotifyPlaylist,
        playlist_items_result: Result<Vec<SourceCollectionItem>, SourceRefreshError>,
        playlist_item_calls: usize,
    }

    impl SpotifySourceApi for FakeSourceApi {
        fn profile(
            &mut self,
            _control: &SourceRefreshControl,
        ) -> Result<SpotifyProfile, SourceRefreshError> {
            Ok(SpotifyProfile {
                provider_account_id: "account".into(),
                display_name: Some("Listener".into()),
                image_url: None,
            })
        }

        fn liked_songs(
            &mut self,
            _control: &SourceRefreshControl,
        ) -> Result<Vec<SourceCollectionItem>, SourceRefreshError> {
            Ok(Vec::new())
        }

        fn playlists(
            &mut self,
            _control: &SourceRefreshControl,
        ) -> Result<Vec<SpotifyPlaylist>, SourceRefreshError> {
            Ok(vec![self.playlist.clone()])
        }

        fn playlist_items(
            &mut self,
            _playlist_id: &str,
            _control: &SourceRefreshControl,
        ) -> Result<Vec<SourceCollectionItem>, SourceRefreshError> {
            self.playlist_item_calls += 1;
            self.playlist_items_result.clone()
        }
    }

    fn fake_playlist(snapshot: &str) -> SpotifyPlaylist {
        SpotifyPlaylist {
            id: "playlist".into(),
            name: "Playlist".into(),
            snapshot_id: Some(snapshot.into()),
            owner_provider_id: Some("account".into()),
            item_count: Some(1),
            image_url: Some("https://i.scdn.co/image/playlist".into()),
            external_url: Some("https://open.spotify.com/playlist/playlist".into()),
        }
    }

    #[test]
    fn unchanged_snapshot_reuses_complete_persisted_entries() {
        let (_path, database, _account_id) = setup_existing_playlist("snapshot", "same");
        let mut api = FakeSourceApi {
            playlist: fake_playlist("same"),
            playlist_items_result: Ok(vec![sample_item("new")]),
            playlist_item_calls: 0,
        };

        let summary = refresh_with_api(
            &database,
            &mut api,
            "client",
            &SourceRefreshControl::default(),
            |_| {},
        )
        .expect("refresh should work");

        assert_eq!(api.playlist_item_calls, 0);
        assert_eq!(summary.unchanged_playlists, 1);
        assert_eq!(summary.refreshed_playlists, 0);
    }

    #[test]
    fn forbidden_playlist_is_marked_inaccessible_without_erasing_entries() {
        let (_path, database, account_id) = setup_existing_playlist("forbidden", "old");
        let mut api = FakeSourceApi {
            playlist: fake_playlist("new"),
            playlist_items_result: Err(SourceRefreshError::new("spotifyForbidden", "forbidden")),
            playlist_item_calls: 0,
        };

        let summary = refresh_with_api(
            &database,
            &mut api,
            "client",
            &SourceRefreshControl::default(),
            |_| {},
        )
        .expect("inaccessible playlist should not fail the refresh");
        let state = database
            .source_collection_state(account_id, "playlist")
            .expect("state should load")
            .expect("playlist should exist");

        assert_eq!(summary.inaccessible_playlists, 1);
        assert!(!state.is_accessible);
        assert_eq!(state.snapshot_id.as_deref(), Some("new"));
        assert_eq!(state.entry_count, 1);
    }

    #[test]
    fn network_failure_preserves_previous_playlist_snapshot_and_entries() {
        let (_path, database, account_id) = setup_existing_playlist("network", "old");
        let mut api = FakeSourceApi {
            playlist: fake_playlist("new"),
            playlist_items_result: Err(SourceRefreshError::new(
                "spotifyNetworkFailed",
                "network failed",
            )),
            playlist_item_calls: 0,
        };

        let error = refresh_with_api(
            &database,
            &mut api,
            "client",
            &SourceRefreshControl::default(),
            |_| {},
        )
        .expect_err("refresh should fail");
        let state = database
            .source_collection_state(account_id, "playlist")
            .expect("state should load")
            .expect("playlist should exist");

        assert_eq!(error.code, "spotifyNetworkFailed");
        assert!(state.is_accessible);
        assert_eq!(state.snapshot_id.as_deref(), Some("old"));
        assert_eq!(state.entry_count, 1);
    }

    #[test]
    fn timestamp_parser_handles_spotify_utc_values() {
        assert_eq!(parse_spotify_timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_spotify_timestamp("1970-01-01T00:00:01.250Z"),
            Some(1_250)
        );
    }
}
