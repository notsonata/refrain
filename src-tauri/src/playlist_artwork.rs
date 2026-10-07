use std::{io::Read, time::Duration};

use reqwest::{
    blocking::Client,
    header::{ACCEPT, CONTENT_TYPE},
};

const ARTWORK_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_ARTWORK_BYTES: u64 = 12 * 1024 * 1024;

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
    use super::artwork_extension;

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
