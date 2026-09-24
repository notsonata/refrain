use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use reqwest::header::RETRY_AFTER;
use serde::Deserialize;

use crate::{
    db::{Database, DatabaseError},
    domain::{SourceCollection, SourceCollectionItem, SourceTrack},
    source_sync::SourceRefreshError,
    spotify::{SpotifyAuthError, SpotifyClient},
};

const SPOTIFY_API_BASE: &str = "https://api.spotify.com/v1";
const PAGE_LIMIT: usize = 50;
const MAX_REQUEST_ATTEMPTS: usize = 3;
const MAX_RATE_LIMIT_WAIT_SECONDS: u64 = 60;

static SAVED_ALBUM_REFRESH_CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn prepare_spotify_saved_album_refresh() {
    SAVED_ALBUM_REFRESH_CANCELLED.store(false, Ordering::Release);
}

pub fn cancel_spotify_saved_album_refresh() {
    SAVED_ALBUM_REFRESH_CANCELLED.store(true, Ordering::Release);
}

pub fn refresh_spotify_saved_albums(
    database: &Database,
    spotify: Arc<SpotifyClient>,
    client_id: &str,
) -> Result<usize, SourceRefreshError> {
    check_cancelled()?;
    let source_account_id = database
        .latest_spotify_source_account_id()
        .map_err(|error| database_error("load Spotify account", error))?
        .ok_or_else(|| {
            SourceRefreshError::new(
                "persistenceFailed",
                "Spotify account state is unavailable. Refresh Spotify again.",
            )
        })?;
    let access_token = spotify.access_token(client_id).map_err(source_auth_error)?;
    let transport = ReqwestSavedAlbumTransport::new()?;
    let albums = load_saved_albums(&transport, &access_token, SPOTIFY_API_BASE, source_account_id)?;
    check_cancelled()?;
    let count = albums.len();
    database
        .replace_spotify_saved_albums(source_account_id, &albums)
        .map_err(|error| database_error("save Spotify saved albums", error))?;
    Ok(count)
}

fn load_saved_albums<T: SavedAlbumTransport>(
    transport: &T,
    access_token: &str,
    api_base: &str,
    source_account_id: i64,
) -> Result<Vec<(SourceCollection, Vec<SourceCollectionItem>)>, SourceRefreshError> {
    let mut next = Some(format!(
        "{}/me/albums?limit={PAGE_LIMIT}&offset=0",
        api_base.trim_end_matches('/')
    ));
    let mut albums = Vec::new();

    while let Some(url) = next {
        check_cancelled()?;
        let page = request_json::<SpotifyPage<SpotifySavedAlbumDto>, _>(
            transport,
            &url,
            access_token,
        )?;
        for saved in page.items {
            check_cancelled()?;
            let album = saved.album.ok_or_else(|| {
                SourceRefreshError::new(
                    "spotifyInvalidResponse",
                    "Spotify returned a saved album without album metadata.",
                )
            })?;
            albums.push(map_saved_album(
                transport,
                access_token,
                source_account_id,
                saved.added_at.as_deref().and_then(parse_spotify_timestamp),
                album,
            )?);
        }
        next = page.next;
    }

    Ok(albums)
}

fn map_saved_album<T: SavedAlbumTransport>(
    transport: &T,
    access_token: &str,
    source_account_id: i64,
    added_at: Option<i64>,
    album: SpotifySavedAlbumObjectDto,
) -> Result<(SourceCollection, Vec<SourceCollectionItem>), SourceRefreshError> {
    let album_id = album.id.clone().ok_or_else(|| {
        SourceRefreshError::new(
            "spotifyInvalidResponse",
            "Spotify returned a saved album without an identifier.",
        )
    })?;
    let album_name = album
        .name
        .clone()
        .unwrap_or_else(|| "Untitled album".into());
    let album_image_url = album.images.iter().find_map(|image| image.url.clone());
    let release_year = album.release_date.as_deref().and_then(parse_release_year);
    let mut items = Vec::new();

    for track in album.tracks.items {
        check_cancelled()?;
        let position = items.len() as i64;
        items.push(map_album_track(
            track,
            position,
            added_at,
            &album_name,
            release_year,
            album_image_url.as_deref(),
        ));
    }

    let mut next = album.tracks.next;
    while let Some(url) = next {
        check_cancelled()?;
        let page = request_json::<SpotifyPage<SpotifySimplifiedTrackDto>, _>(
            transport,
            &url,
            access_token,
        )?;
        for track in page.items {
            check_cancelled()?;
            let position = items.len() as i64;
            items.push(map_album_track(
                track,
                position,
                added_at,
                &album_name,
                release_year,
                album_image_url.as_deref(),
            ));
        }
        next = page.next;
    }

    Ok((
        SourceCollection {
            source_account_id,
            provider_collection_id: format!("spotify:album:{album_id}"),
            kind: "saved_album".into(),
            name: album_name,
            snapshot_id: None,
            owner_provider_id: None,
            is_accessible: true,
            access_issue: None,
        },
        items,
    ))
}

fn map_album_track(
    track: SpotifySimplifiedTrackDto,
    position: i64,
    added_at: Option<i64>,
    album_name: &str,
    release_year: Option<i64>,
    album_image_url: Option<&str>,
) -> SourceCollectionItem {
    let provider_item_uri = track.uri.clone();
    let mapped_track = map_simplified_track(&track, album_name, release_year, album_image_url);
    let unavailable_reason = mapped_track.is_none().then(|| {
        if track.is_local.unwrap_or(false) {
            "local_track"
        } else {
            "unavailable_track"
        }
        .into()
    });

    SourceCollectionItem {
        position,
        track: mapped_track,
        provider_item_uri,
        item_type: "track".into(),
        added_at,
        unavailable_reason,
    }
}

fn map_simplified_track(
    track: &SpotifySimplifiedTrackDto,
    album_name: &str,
    release_year: Option<i64>,
    album_image_url: Option<&str>,
) -> Option<SourceTrack> {
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

    Some(SourceTrack {
        provider: "spotify".into(),
        provider_track_id,
        uri: track.uri.clone(),
        isrc: None,
        normalized_title: normalize_source_text(&title),
        title,
        artists_json,
        normalized_artists,
        normalized_album: Some(normalize_source_text(album_name)),
        album: Some(album_name.into()),
        duration_ms: track.duration_ms,
        disc_number: track.disc_number,
        track_number: track.track_number,
        release_year,
        explicit: track.explicit,
        version_kind: None,
        version_detail: None,
        image_url: album_image_url.map(str::to_owned),
        external_url: track
            .external_urls
            .as_ref()
            .and_then(|urls| urls.spotify.clone()),
    })
}

trait SavedAlbumTransport {
    fn get(&self, url: &str, bearer_token: &str) -> Result<SavedAlbumHttpResponse, SourceRefreshError>;
}

struct ReqwestSavedAlbumTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestSavedAlbumTransport {
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

impl SavedAlbumTransport for ReqwestSavedAlbumTransport {
    fn get(&self, url: &str, bearer_token: &str) -> Result<SavedAlbumHttpResponse, SourceRefreshError> {
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
        Ok(SavedAlbumHttpResponse {
            status,
            retry_after_seconds,
            body,
        })
    }
}

#[derive(Debug, Clone)]
struct SavedAlbumHttpResponse {
    status: u16,
    retry_after_seconds: Option<u64>,
    body: String,
}

fn request_json<D: for<'de> Deserialize<'de>, T: SavedAlbumTransport>(
    transport: &T,
    url: &str,
    access_token: &str,
) -> Result<D, SourceRefreshError> {
    for attempt in 0..MAX_REQUEST_ATTEMPTS {
        check_cancelled()?;
        let response = match transport.get(url, access_token) {
            Ok(response) => response,
            Err(error)
                if error.code == "spotifyNetworkFailed" && attempt + 1 < MAX_REQUEST_ATTEMPTS =>
            {
                sleep_interruptibly(Duration::from_millis(250 * (attempt as u64 + 1)))?;
                continue;
            }
            Err(error) => return Err(error),
        };

        match response.status {
            200..=299 => {
                return serde_json::from_str(&response.body).map_err(|error| {
                    SourceRefreshError::new(
                        "spotifyInvalidResponse",
                        format!("Spotify returned an invalid saved-album response: {error}"),
                    )
                });
            }
            401 => {
                return Err(SourceRefreshError::new(
                    "reauthorizationRequired",
                    "Spotify rejected the access token while loading saved albums. Connect Spotify again.",
                ));
            }
            403 => {
                return Err(SourceRefreshError::new(
                    "spotifyForbidden",
                    "Spotify does not allow access to saved albums for this account.",
                ));
            }
            429 if attempt + 1 < MAX_REQUEST_ATTEMPTS => {
                let retry_after = response.retry_after_seconds.unwrap_or(1);
                if retry_after > MAX_RATE_LIMIT_WAIT_SECONDS {
                    return Err(SourceRefreshError::new(
                        "spotifyRateLimited",
                        format!(
                            "Spotify asked Refrain to retry after {retry_after} seconds. Try again later."
                        ),
                    ));
                }
                sleep_interruptibly(Duration::from_secs(retry_after))?;
            }
            429 => {
                return Err(SourceRefreshError::new(
                    "spotifyRateLimited",
                    "Spotify rate limiting prevented the saved-album refresh. Try again later.",
                ));
            }
            500..=599 if attempt + 1 < MAX_REQUEST_ATTEMPTS => {
                sleep_interruptibly(Duration::from_millis(250 * (attempt as u64 + 1)))?;
            }
            status => {
                return Err(SourceRefreshError::new(
                    "spotifyRequestFailed",
                    format!("Spotify saved-album request failed with HTTP {status}."),
                ));
            }
        }
    }

    Err(SourceRefreshError::new(
        "spotifyRequestFailed",
        "Spotify saved-album request failed after retrying.",
    ))
}

fn check_cancelled() -> Result<(), SourceRefreshError> {
    if SAVED_ALBUM_REFRESH_CANCELLED.load(Ordering::Acquire) {
        return Err(SourceRefreshError::new(
            "refreshCancelled",
            "Spotify source refresh was cancelled.",
        ));
    }
    Ok(())
}

fn sleep_interruptibly(duration: Duration) -> Result<(), SourceRefreshError> {
    let mut remaining = duration;
    let slice = Duration::from_millis(100);
    while !remaining.is_zero() {
        check_cancelled()?;
        let current = remaining.min(slice);
        thread::sleep(current);
        remaining = remaining.saturating_sub(current);
    }
    check_cancelled()
}

fn source_auth_error(error: SpotifyAuthError) -> SourceRefreshError {
    SourceRefreshError::new(error.code, error.message)
}

fn database_error(operation: &str, error: DatabaseError) -> SourceRefreshError {
    tracing::error!(%error, operation, "Spotify saved album persistence failed");
    SourceRefreshError::new("persistenceFailed", format!("Failed to {operation}."))
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct SpotifyPage<T> {
    #[serde(default)]
    items: Vec<T>,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifySavedAlbumDto {
    added_at: Option<String>,
    album: Option<SpotifySavedAlbumObjectDto>,
}

#[derive(Debug, Deserialize)]
struct SpotifySavedAlbumObjectDto {
    id: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    #[serde(default)]
    images: Vec<SpotifyImageDto>,
    tracks: SpotifyPage<SpotifySimplifiedTrackDto>,
}

#[derive(Debug, Deserialize)]
struct SpotifySimplifiedTrackDto {
    id: Option<String>,
    uri: Option<String>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<SpotifyArtistDto>,
    duration_ms: Option<i64>,
    disc_number: Option<i64>,
    track_number: Option<i64>,
    explicit: Option<bool>,
    external_urls: Option<SpotifyExternalUrlsDto>,
    is_local: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtistDto {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyImageDto {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyExternalUrlsDto {
    spotify: Option<String>,
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
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;

    struct MockTransport {
        responses: Mutex<VecDeque<Result<SavedAlbumHttpResponse, SourceRefreshError>>>,
    }

    impl MockTransport {
        fn with_responses(
            responses: impl IntoIterator<Item = Result<SavedAlbumHttpResponse, SourceRefreshError>>,
        ) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
            }
        }
    }

    impl SavedAlbumTransport for MockTransport {
        fn get(
            &self,
            _url: &str,
            _bearer_token: &str,
        ) -> Result<SavedAlbumHttpResponse, SourceRefreshError> {
            self.responses
                .lock()
                .expect("responses should lock")
                .pop_front()
                .expect("mock response should exist")
        }
    }

    fn response(body: &str) -> Result<SavedAlbumHttpResponse, SourceRefreshError> {
        Ok(SavedAlbumHttpResponse {
            status: 200,
            retry_after_seconds: None,
            body: body.into(),
        })
    }

    #[test]
    fn saved_album_pagination_preserves_album_track_order() {
        prepare_spotify_saved_album_refresh();
        let transport = MockTransport::with_responses([
            response(
                r#"{"items":[{"added_at":"2026-09-01T00:00:00Z","album":{"id":"album-one","name":"Album One","release_date":"2024-02-03","images":[{"url":"https://i.scdn.co/image/cover"}],"tracks":{"items":[{"id":"track-one","uri":"spotify:track:track-one","name":"One","artists":[{"name":"Artist"}],"duration_ms":180000,"disc_number":1,"track_number":1,"explicit":false}],"next":"https://api.test/albums/album-one/tracks?page=2"}}}],"next":null}"#,
            ),
            response(
                r#"{"items":[{"id":"track-two","uri":"spotify:track:track-two","name":"Two","artists":[{"name":"Artist"}],"duration_ms":190000,"disc_number":1,"track_number":2,"explicit":false}],"next":null}"#,
            ),
        ]);

        let albums = load_saved_albums(&transport, "token", "https://api.test", 7)
            .expect("saved albums should load");

        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].0.provider_collection_id, "spotify:album:album-one");
        assert_eq!(albums[0].0.kind, "saved_album");
        assert_eq!(albums[0].1.len(), 2);
        assert_eq!(albums[0].1[0].position, 0);
        assert_eq!(albums[0].1[1].position, 1);
        assert_eq!(
            albums[0].1[1]
                .track
                .as_ref()
                .map(|track| track.provider_track_id.as_str()),
            Some("track-two")
        );
        assert_eq!(
            albums[0].1[0]
                .track
                .as_ref()
                .and_then(|track| track.image_url.as_deref()),
            Some("https://i.scdn.co/image/cover")
        );
    }

    #[test]
    fn cancellation_stops_saved_album_refresh() {
        prepare_spotify_saved_album_refresh();
        cancel_spotify_saved_album_refresh();
        let transport = MockTransport::with_responses([]);

        let error = load_saved_albums(&transport, "token", "https://api.test", 7)
            .expect_err("cancelled refresh should stop");

        assert_eq!(error.code, "refreshCancelled");
        prepare_spotify_saved_album_refresh();
    }
}
