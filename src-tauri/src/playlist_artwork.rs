use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::db::Database;
use reqwest::{
    blocking::Client,
    header::{ACCEPT, CONTENT_TYPE},
};

const ARTWORK_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_ARTWORK_BYTES: u64 = 12 * 1024 * 1024;
pub(crate) const PLAYLIST_DIRECTORY: &str = "00 Playlists";

pub(crate) fn custom_artwork_directory(library_root: &Path) -> PathBuf {
    library_root.join(PLAYLIST_DIRECTORY)
}

pub(crate) fn configured_custom_artwork_directory(database: &Database) -> Result<PathBuf, String> {
    let root = database
        .get_settings()
        .map_err(|error| format!("Could not read library settings: {error}"))?
        .library_root
        .filter(|root| !root.trim().is_empty())
        .ok_or_else(|| "Choose a library folder before replacing playlist artwork.".to_owned())?;
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return Err("The library folder must be an absolute path.".to_owned());
    }
    Ok(custom_artwork_directory(&root))
}

pub(crate) fn import_custom_playlist_artwork(
    directory: &Path,
    playlist_id: i64,
    source: &Path,
) -> Result<PathBuf, String> {
    let file = fs::File::open(source)
        .map_err(|error| format!("Could not open selected image: {error}"))?;
    if file
        .metadata()
        .map_err(|error| format!("Could not inspect selected image: {error}"))?
        .len()
        > MAX_ARTWORK_BYTES
    {
        return Err("Playlist artwork is larger than the 12 MiB limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read selected image: {error}"))?;
    if bytes.len() as u64 > MAX_ARTWORK_BYTES {
        return Err("Playlist artwork is larger than the 12 MiB limit.".into());
    }
    let extension = artwork_extension(&bytes, None)
        .ok_or_else(|| "Select a JPEG, PNG, WebP, or GIF image.".to_owned())?;

    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create playlist artwork folder: {error}"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Could not generate artwork filename: {error}"))?
        .as_nanos();
    let destination = directory.join(format!("refrain-artwork-{playlist_id}-{nonce}.{extension}"));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|error| format!("Could not create playlist artwork file: {error}"))?;
    if let Err(error) = output.write_all(&bytes) {
        drop(output);
        let _ = fs::remove_file(&destination);
        return Err(format!("Could not save playlist artwork: {error}"));
    }
    Ok(destination)
}

pub(crate) fn remove_custom_playlist_artwork(
    app_data_dir: &Path,
    playlist_id: i64,
    stored_path: &str,
) {
    let path = Path::new(stored_path);
    if is_owned_custom_playlist_artwork(app_data_dir, playlist_id, path)
        && let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, %playlist_id, "could not clean up replaced playlist artwork");
    }
}

pub(crate) fn is_owned_custom_playlist_artwork(
    app_data_dir: &Path,
    playlist_id: i64,
    path: &Path,
) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let old_owned = path.parent() == Some(app_data_dir.join("playlist-artwork").as_path())
        && name.starts_with(&format!("{playlist_id}-"));
    let new_owned = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        == Some(PLAYLIST_DIRECTORY)
        && name.starts_with(&format!("refrain-artwork-{playlist_id}-"));
    (old_owned || new_owned)
        && matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("jpg" | "png" | "webp" | "gif")
        )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadedPlaylistArtwork {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

pub(crate) fn download_playlist_artwork(
    image_url: &str,
) -> Result<DownloadedPlaylistArtwork, String> {
    let url = url::Url::parse(image_url)
        .map_err(|error| format!("The playlist artwork URL is invalid: {error}"))?;
    if url.scheme() != "https" {
        return Err("The playlist artwork URL must use HTTPS.".into());
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(ARTWORK_DOWNLOAD_TIMEOUT)
        .user_agent(concat!("Refrain/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Could not initialize playlist artwork download: {error}"))?;
    let response = client
        .get(url)
        .header(ACCEPT, "image/*")
        .send()
        .map_err(|error| format!("Could not download playlist artwork: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Playlist artwork download returned HTTP {}.",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_ARTWORK_BYTES)
    {
        return Err("Playlist artwork is larger than the 12 MiB limit.".into());
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut bytes = Vec::new();
    response
        .take(MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read playlist artwork: {error}"))?;
    if bytes.len() as u64 > MAX_ARTWORK_BYTES {
        return Err("Playlist artwork is larger than the 12 MiB limit.".into());
    }

    let extension = artwork_extension(&bytes, content_type.as_deref())
        .ok_or_else(|| "Playlist artwork is not a supported image format.".to_owned())?;
    Ok(DownloadedPlaylistArtwork { bytes, extension })
}

fn artwork_extension(bytes: &[u8], content_type: Option<&str>) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }

    match content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
    {
        Some("image/jpeg") => Some("jpg"),
        Some("image/png") => Some("png"),
        Some("image/webp") => Some("webp"),
        Some("image/gif") => Some("gif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        artwork_extension, custom_artwork_directory, import_custom_playlist_artwork,
        remove_custom_playlist_artwork,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn imported_artwork_is_copied_validated_and_removed_only_from_owned_directory() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("refrain-cover-test-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("original.png");
        fs::write(&source, b"\x89PNG\r\n\x1a\nexample").unwrap();
        let directory = custom_artwork_directory(&root);
        let imported = import_custom_playlist_artwork(&directory, 7, &source).unwrap();
        assert!(imported.exists());
        assert_eq!(imported.parent(), Some(directory.as_path()));
        assert_eq!(fs::read(&imported).unwrap(), fs::read(&source).unwrap());
        assert_ne!(imported, source);

        remove_custom_playlist_artwork(&root, 8, &imported.to_string_lossy());
        assert!(
            imported.exists(),
            "other playlist IDs cannot remove the artwork"
        );
        remove_custom_playlist_artwork(&root, 7, &source.to_string_lossy());
        assert!(source.exists(), "original files must remain untouched");
        remove_custom_playlist_artwork(&root, 7, &imported.to_string_lossy());
        assert!(!imported.exists());

        let legacy = root.join("playlist-artwork/7-123.png");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"\x89PNG\r\n\x1a\nlegacy").unwrap();
        remove_custom_playlist_artwork(&root, 8, &legacy.to_string_lossy());
        assert!(legacy.exists());
        remove_custom_playlist_artwork(&root, 7, &legacy.to_string_lossy());
        assert!(!legacy.exists());

        fs::write(&source, b"not an image").unwrap();
        assert!(import_custom_playlist_artwork(&directory, 7, &source).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_supported_playlist_artwork_formats() {
        assert_eq!(
            artwork_extension(&[0xff, 0xd8, 0xff, 0x00], None),
            Some("jpg")
        );
        assert_eq!(
            artwork_extension(b"\x89PNG\r\n\x1a\nrest", None),
            Some("png")
        );
        assert_eq!(artwork_extension(b"RIFFxxxxWEBPrest", None), Some("webp"));
        assert_eq!(
            artwork_extension(b"unknown", Some("image/jpeg")),
            Some("jpg")
        );
    }
}
