use std::collections::{BTreeSet, HashMap, HashSet};

use strsim::normalized_levenshtein;
use unicode_normalization::UnicodeNormalization;

use crate::domain::{
    MatchCandidateEvidence, MatchOutcome, MatchRelationship, MatchResult, MatchScoreBreakdown,
    MatchTrackDescriptor,
};

const AUTO_THRESHOLD: i64 = 9_200;
const REVIEW_THRESHOLD: i64 = 8_000;
const RUNNER_UP_MARGIN: i64 = 800;
const MAX_INDEXED_CANDIDATES: usize = 200;
const MAX_FUZZY_CANDIDATES: usize = 25;
const DURATION_BUCKET_MS: i64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistedTrackLink {
    pub library_track_id: i64,
    pub method: String,
    pub confidence: i64,
    pub confirmed_by_user: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum VersionClass {
    Live,
    Acoustic,
    Remix,
    Demo,
    Instrumental,
    RadioEdit,
    Extended,
    Clean,
    Explicit,
    Remaster,
    Mono,
    Stereo,
    Other,
}

#[derive(Debug, Clone)]
struct NormalizedTrack {
    title: String,
    artists: BTreeSet<String>,
    primary_artist: Option<String>,
    album: Option<String>,
    versions: BTreeSet<VersionClass>,
}

#[derive(Debug, Clone)]
struct IndexedTrack {
    descriptor: MatchTrackDescriptor,
    normalized: NormalizedTrack,
}

#[derive(Debug)]
pub(crate) struct MatcherIndex {
    tracks: Vec<IndexedTrack>,
    by_isrc: HashMap<String, Vec<usize>>,
    by_title: HashMap<String, Vec<usize>>,
    by_primary_artist: HashMap<String, Vec<usize>>,
    by_duration_bucket: HashMap<i64, Vec<usize>>,
    by_title_trigram: HashMap<String, Vec<usize>>,
}

#[derive(Debug)]
struct ScoredCandidate {
    evidence: MatchCandidateEvidence,
    eligible: bool,
}

#[derive(Debug, Clone, Copy)]
struct DurationEvidence {
    score: f64,
    difference_ms: Option<i64>,
    incompatible: bool,
    outside_soft_tolerance: bool,
}

impl MatcherIndex {
    pub(crate) fn new(mut tracks: Vec<MatchTrackDescriptor>) -> Self {
        tracks.sort_by_key(|track| track.id);
        let tracks = tracks
            .into_iter()
            .map(|descriptor| {
                let normalized = normalize_track(&descriptor);
                IndexedTrack {
                    descriptor,
                    normalized,
                }
            })
            .collect::<Vec<_>>();
        let mut index = Self {
            tracks,
            by_isrc: HashMap::new(),
            by_title: HashMap::new(),
            by_primary_artist: HashMap::new(),
            by_duration_bucket: HashMap::new(),
            by_title_trigram: HashMap::new(),
        };
        for (position, track) in index.tracks.iter().enumerate() {
            if let Some(isrc) = normalized_isrc(track.descriptor.isrc.as_deref()) {
                index.by_isrc.entry(isrc).or_default().push(position);
            }
            if !track.normalized.title.is_empty() {
                index
                    .by_title
                    .entry(track.normalized.title.clone())
                    .or_default()
                    .push(position);
                for trigram in title_trigrams(&track.normalized.title) {
                    index
                        .by_title_trigram
                        .entry(trigram)
                        .or_default()
                        .push(position);
                }
            }
            if let Some(artist) = track.normalized.primary_artist.as_ref() {
                index
                    .by_primary_artist
                    .entry(artist.clone())
                    .or_default()
                    .push(position);
            }
            if let Some(duration) = track.descriptor.duration_ms {
                index
                    .by_duration_bucket
                    .entry(duration / DURATION_BUCKET_MS)
                    .or_default()
                    .push(position);
            }
        }
        index
    }

    pub(crate) fn match_track(
        &self,
        source: &MatchTrackDescriptor,
        existing_link: Option<&PersistedTrackLink>,
        rejected_library_track_ids: &HashSet<i64>,
    ) -> MatchResult {
        let mut rejected_ids = rejected_library_track_ids
            .iter()
            .copied()
            .collect::<Vec<_>>();
        rejected_ids.sort_unstable();

        if let Some(link) = existing_link
            && self
                .tracks
                .iter()
                .any(|track| track.descriptor.id == link.library_track_id)
        {
            return MatchResult {
                source_track_id: source.id,
                outcome: MatchOutcome::Automatic,
                selected_library_track_id: Some(link.library_track_id),
                method: Some(if link.confirmed_by_user {
                    "user".into()
                } else {
                    link.method.clone()
                }),
                confidence: Some(link.confidence),
                candidates: Vec::new(),
                rejected_library_track_ids: rejected_ids,
            };
        }

        let source_normalized = normalize_track(source);
        let candidate_positions = self.candidate_positions(source, &source_normalized);
        let mut scored = candidate_positions
            .into_iter()
            .map(|position| {
                self.score_candidate(
                    source,
                    &source_normalized,
                    &self.tracks[position],
                    rejected_library_track_ids,
                )
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .evidence
                .score
                .total
                .cmp(&left.evidence.score.total)
                .then_with(|| {
                    left.evidence
                        .library_track_id
                        .cmp(&right.evidence.library_track_id)
                })
        });

        let exact_isrc_count = scored
            .iter()
            .filter(|candidate| candidate.eligible && candidate.evidence.exact_isrc)
            .count();

        let eligible = scored
            .iter()
            .filter(|candidate| candidate.eligible)
            .collect::<Vec<_>>();
        let top = eligible.first().copied();
        let runner_up = eligible.get(1).copied();

        let (outcome, selected_library_track_id, method, confidence) = if let Some(top) = top {
            let exact_isrc_auto = top.evidence.exact_isrc
                && exact_isrc_count == 1
                && top.evidence.incompatibilities.is_empty();
            let margin = runner_up
                .map(|runner_up| top.evidence.score.total - runner_up.evidence.score.total)
                .unwrap_or(i64::MAX);
            let metadata_auto = top.evidence.score.total >= AUTO_THRESHOLD
                && margin >= RUNNER_UP_MARGIN
                && top.evidence.warnings.is_empty();

            if exact_isrc_auto || metadata_auto {
                (
                    MatchOutcome::Automatic,
                    Some(top.evidence.library_track_id),
                    Some(if exact_isrc_auto { "isrc" } else { "metadata" }.into()),
                    Some(top.evidence.score.total),
                )
            } else if exact_isrc_count > 1 || top.evidence.score.total >= REVIEW_THRESHOLD {
                (
                    MatchOutcome::Review,
                    None,
                    None,
                    Some(top.evidence.score.total),
                )
            } else {
                (MatchOutcome::Unresolved, None, None, None)
            }
        } else {
            (MatchOutcome::Unresolved, None, None, None)
        };

        if outcome == MatchOutcome::Automatic
            && let Some(selected_id) = selected_library_track_id
            && let Some(selected) = scored
                .iter_mut()
                .find(|candidate| candidate.evidence.library_track_id == selected_id)
        {
            selected.evidence.relationship = MatchRelationship::Same;
        }

        MatchResult {
            source_track_id: source.id,
            outcome,
            selected_library_track_id,
            method,
            confidence,
            candidates: scored
                .into_iter()
                .map(|candidate| candidate.evidence)
                .collect(),
            rejected_library_track_ids: rejected_ids,
        }
    }

    fn candidate_positions(
        &self,
        source: &MatchTrackDescriptor,
        normalized: &NormalizedTrack,
    ) -> Vec<usize> {
        let mut positions = BTreeSet::new();
        if let Some(isrc) = normalized_isrc(source.isrc.as_deref())
            && let Some(matches) = self.by_isrc.get(&isrc)
        {
            positions.extend(matches.iter().copied());
        }
        if !normalized.title.is_empty()
            && let Some(matches) = self.by_title.get(&normalized.title)
        {
            positions.extend(matches.iter().copied());
        }
        if let Some(primary_artist) = normalized.primary_artist.as_ref()
            && let Some(matches) = self.by_primary_artist.get(primary_artist)
        {
            positions.extend(matches.iter().copied());
        }
        if let Some(duration) = source.duration_ms {
            let bucket = duration / DURATION_BUCKET_MS;
            for candidate_bucket in (bucket - 2)..=(bucket + 2) {
                if let Some(matches) = self.by_duration_bucket.get(&candidate_bucket) {
                    positions.extend(matches.iter().copied());
                }
            }
        }

        if positions.is_empty() {
            let mut fuzzy_positions = BTreeSet::new();
            for trigram in title_trigrams(&normalized.title) {
                if let Some(matches) = self.by_title_trigram.get(&trigram) {
                    fuzzy_positions.extend(matches.iter().copied());
                }
                if fuzzy_positions.len() >= MAX_INDEXED_CANDIDATES {
                    break;
                }
            }
            let mut fuzzy = fuzzy_positions
                .into_iter()
                .map(|position| {
                    (
                        position,
                        text_similarity(&normalized.title, &self.tracks[position].normalized.title),
                    )
                })
                .filter(|(_, similarity)| *similarity >= 0.45)
                .collect::<Vec<_>>();
            fuzzy.sort_by(|left, right| {
                right.1.total_cmp(&left.1).then_with(|| {
                    self.tracks[left.0]
                        .descriptor
                        .id
                        .cmp(&self.tracks[right.0].descriptor.id)
                })
            });
            return fuzzy
                .into_iter()
                .take(MAX_FUZZY_CANDIDATES)
                .map(|(position, _)| position)
                .collect();
        }

        let mut positions = positions.into_iter().collect::<Vec<_>>();
        if positions.len() > MAX_INDEXED_CANDIDATES {
            positions.sort_by(|left, right| {
                let left_similarity =
                    text_similarity(&normalized.title, &self.tracks[*left].normalized.title);
                let right_similarity =
                    text_similarity(&normalized.title, &self.tracks[*right].normalized.title);
                right_similarity.total_cmp(&left_similarity).then_with(|| {
                    self.tracks[*left]
                        .descriptor
                        .id
                        .cmp(&self.tracks[*right].descriptor.id)
                })
            });
            positions.truncate(MAX_INDEXED_CANDIDATES);
        }
        positions
    }

    fn score_candidate(
        &self,
        source: &MatchTrackDescriptor,
        source_normalized: &NormalizedTrack,
        candidate: &IndexedTrack,
        rejected_library_track_ids: &HashSet<i64>,
    ) -> ScoredCandidate {
        let mut incompatibilities = hard_incompatibilities(
            source,
            source_normalized,
            &candidate.descriptor,
            &candidate.normalized,
        );
        let duration = duration_evidence(source.duration_ms, candidate.descriptor.duration_ms);
        if duration.incompatible {
            incompatibilities.push("durationOutsideHardTolerance".into());
        }

        let mut warnings = Vec::new();
        let source_isrc = normalized_isrc(source.isrc.as_deref());
        let candidate_isrc = normalized_isrc(candidate.descriptor.isrc.as_deref());
        let exact_isrc = source_isrc.is_some() && source_isrc == candidate_isrc;
        if source_isrc.is_some() && candidate_isrc.is_some() && source_isrc != candidate_isrc {
            warnings.push("conflictingIsrc".into());
        }
        if source_normalized.versions.contains(&VersionClass::Remaster)
            != candidate
                .normalized
                .versions
                .contains(&VersionClass::Remaster)
        {
            warnings.push("remasterDifference".into());
        }
        if duration.outside_soft_tolerance {
            warnings.push("durationOutsideSoftTolerance".into());
        }
        if rejected_library_track_ids.contains(&candidate.descriptor.id) {
            incompatibilities.push("userRejected".into());
        }

        let title_score = text_similarity(&source_normalized.title, &candidate.normalized.title);
        let artist_score =
            artist_similarity(&source_normalized.artists, &candidate.normalized.artists);
        let album_score = match (
            source_normalized.album.as_deref(),
            candidate.normalized.album.as_deref(),
        ) {
            (Some(left), Some(right)) => text_similarity(left, right),
            _ => 0.0,
        };
        let track_disc_score = track_disc_similarity(source, &candidate.descriptor);
        let score = MatchScoreBreakdown {
            title: weighted_score(title_score, 40.0),
            artists: weighted_score(artist_score, 25.0),
            duration: weighted_score(duration.score, 20.0),
            album: weighted_score(album_score, 10.0),
            track_disc: weighted_score(track_disc_score, 5.0),
            total: 0,
        };
        let score = MatchScoreBreakdown {
            total: score.title + score.artists + score.duration + score.album + score.track_disc,
            ..score
        };
        let eligible = incompatibilities.is_empty();

        ScoredCandidate {
            evidence: MatchCandidateEvidence {
                library_track_id: candidate.descriptor.id,
                relationship: if eligible {
                    MatchRelationship::Uncertain
                } else {
                    MatchRelationship::Different
                },
                score,
                exact_isrc,
                duration_difference_ms: duration.difference_ms,
                warnings,
                incompatibilities,
            },
            eligible,
        }
    }
}

pub(crate) fn normalize_comparison_text(value: &str) -> String {
    let normalized = value
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let mut output = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        if character.is_alphanumeric() {
            output.push(character);
        } else if character == '&' {
            output.push_str(" and ");
        } else {
            output.push(' ');
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_track(track: &MatchTrackDescriptor) -> NormalizedTrack {
    let (without_feature, featured_artists) = split_featured_suffix(&track.title);
    let (base_title, title_versions) = strip_version_qualifiers(&without_feature);
    let mut versions = title_versions;
    add_versions_from_optional(track.version_kind.as_deref(), &mut versions);
    add_versions_from_optional(track.version_detail.as_deref(), &mut versions);

    let mut normalized_artists = track
        .artists
        .iter()
        .map(|artist| normalize_comparison_text(artist))
        .filter(|artist| !artist.is_empty())
        .collect::<Vec<_>>();
    normalized_artists.extend(
        featured_artists
            .iter()
            .map(|artist| normalize_comparison_text(artist))
            .filter(|artist| !artist.is_empty()),
    );
    let primary_artist = normalized_artists.first().cloned();
    let artists = normalized_artists.into_iter().collect::<BTreeSet<_>>();

    NormalizedTrack {
        title: normalize_comparison_text(&base_title),
        artists,
        primary_artist,
        album: track
            .album
            .as_deref()
            .map(normalize_comparison_text)
            .filter(|album| !album.is_empty()),
        versions,
    }
}

fn split_featured_suffix(value: &str) -> (String, Vec<String>) {
    let normalized = value.nfkc().collect::<String>();
    let lowercase = normalized.to_lowercase();
    let markers = [
        " (feat. ",
        " [feat. ",
        " feat. ",
        " (ft. ",
        " [ft. ",
        " ft. ",
        " (featuring ",
        " [featuring ",
        " featuring ",
    ];
    let match_position = markers
        .iter()
        .filter_map(|marker| {
            lowercase
                .find(marker)
                .map(|position| (position, marker.len()))
        })
        .min_by_key(|(position, _)| *position);
    let Some((position, marker_length)) = match_position else {
        return (normalized, Vec::new());
    };
    let featured = normalized[position + marker_length..]
        .trim_end_matches([')', ']'])
        .trim();
    let artists = split_featured_artists(featured);
    (normalized[..position].trim().to_owned(), artists)
}

fn split_featured_artists(value: &str) -> Vec<String> {
    let mut normalized = value.replace('&', ",");
    for connector in [" and ", " x ", ";", "/"] {
        normalized = normalized.replace(connector, ",");
    }
    normalized
        .split(',')
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .map(str::to_owned)
        .collect()
}

fn strip_version_qualifiers(value: &str) -> (String, BTreeSet<VersionClass>) {
    let mut working = value.to_owned();
    let mut versions = BTreeSet::new();

    while let Some((start, open, close)) = first_bracket(&working) {
        let Some(relative_end) = working[start + open.len_utf8()..].find(close) else {
            break;
        };
        let end = start + open.len_utf8() + relative_end;
        let segment = &working[start + open.len_utf8()..end];
        let segment_versions = version_classes(segment);
        if segment_versions.is_empty() {
            let next_start = end + close.len_utf8();
            if next_start >= working.len() {
                break;
            }
            let prefix = working[..next_start].to_owned();
            let suffix = working[next_start..].to_owned();
            let (stripped_suffix, suffix_versions) = strip_version_qualifiers(&suffix);
            versions.extend(suffix_versions);
            return (format!("{prefix}{stripped_suffix}"), versions);
        }
        versions.extend(segment_versions);
        working.replace_range(start..end + close.len_utf8(), " ");
    }

    for separator in [" - ", " – ", " — "] {
        if let Some(position) = working.rfind(separator) {
            let suffix = &working[position + separator.len()..];
            let suffix_versions = version_classes(suffix);
            if !suffix_versions.is_empty() {
                versions.extend(suffix_versions);
                working.truncate(position);
                break;
            }
        }
    }

    (working.trim().to_owned(), versions)
}

fn first_bracket(value: &str) -> Option<(usize, char, char)> {
    let round = value.find('(').map(|position| (position, '(', ')'));
    let square = value.find('[').map(|position| (position, '[', ']'));
    match (round, square) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn add_versions_from_optional(value: Option<&str>, versions: &mut BTreeSet<VersionClass>) {
    if let Some(value) = value {
        let detected = version_classes(value);
        if detected.is_empty() && !value.trim().is_empty() {
            versions.insert(VersionClass::Other);
        } else {
            versions.extend(detected);
        }
    }
}

fn version_classes(value: &str) -> BTreeSet<VersionClass> {
    let text = normalize_comparison_text(value);
    let mut versions = BTreeSet::new();
    let checks = [
        (VersionClass::Live, ["live", "live version"].as_slice()),
        (VersionClass::Acoustic, ["acoustic"].as_slice()),
        (VersionClass::Remix, ["remix"].as_slice()),
        (VersionClass::Demo, ["demo"].as_slice()),
        (VersionClass::Instrumental, ["instrumental"].as_slice()),
        (
            VersionClass::RadioEdit,
            ["radio edit", "radio version"].as_slice(),
        ),
        (
            VersionClass::Extended,
            ["extended", "extended mix"].as_slice(),
        ),
        (VersionClass::Clean, ["clean", "clean version"].as_slice()),
        (
            VersionClass::Explicit,
            ["explicit", "explicit version"].as_slice(),
        ),
        (
            VersionClass::Remaster,
            ["remaster", "remastered"].as_slice(),
        ),
        (VersionClass::Mono, ["mono"].as_slice()),
        (VersionClass::Stereo, ["stereo"].as_slice()),
    ];
    for (version, phrases) in checks {
        if phrases.iter().any(|phrase| contains_phrase(&text, phrase)) {
            versions.insert(version);
        }
    }
    versions
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    text == phrase
        || text.starts_with(&format!("{phrase} "))
        || text.ends_with(&format!(" {phrase}"))
        || text.contains(&format!(" {phrase} "))
}

fn hard_incompatibilities(
    source: &MatchTrackDescriptor,
    source_normalized: &NormalizedTrack,
    candidate: &MatchTrackDescriptor,
    candidate_normalized: &NormalizedTrack,
) -> Vec<String> {
    let mut conflicts = Vec::new();
    for (version, label) in [
        (VersionClass::Live, "liveStudioConflict"),
        (VersionClass::Acoustic, "acousticConflict"),
        (VersionClass::Remix, "remixConflict"),
        (VersionClass::Demo, "demoConflict"),
        (VersionClass::Instrumental, "instrumentalConflict"),
        (VersionClass::RadioEdit, "radioEditConflict"),
        (VersionClass::Extended, "extendedConflict"),
    ] {
        if source_normalized.versions.contains(&version)
            != candidate_normalized.versions.contains(&version)
        {
            conflicts.push(label.into());
        }
    }
    if source.explicit.is_some()
        && candidate.explicit.is_some()
        && source.explicit != candidate.explicit
    {
        conflicts.push("explicitCleanConflict".into());
    }
    if (source_normalized.versions.contains(&VersionClass::Explicit)
        && candidate_normalized.versions.contains(&VersionClass::Clean))
        || (source_normalized.versions.contains(&VersionClass::Clean)
            && candidate_normalized
                .versions
                .contains(&VersionClass::Explicit))
    {
        conflicts.push("explicitCleanConflict".into());
    }
    if (source_normalized.versions.contains(&VersionClass::Mono)
        && candidate_normalized
            .versions
            .contains(&VersionClass::Stereo))
        || (source_normalized.versions.contains(&VersionClass::Stereo)
            && candidate_normalized.versions.contains(&VersionClass::Mono))
    {
        conflicts.push("monoStereoConflict".into());
    }
    conflicts.sort();
    conflicts.dedup();
    conflicts
}

fn duration_evidence(source: Option<i64>, candidate: Option<i64>) -> DurationEvidence {
    let (Some(source), Some(candidate)) = (source, candidate) else {
        return DurationEvidence {
            score: 0.0,
            difference_ms: None,
            incompatible: false,
            outside_soft_tolerance: false,
        };
    };
    let difference = source.abs_diff(candidate) as i64;
    let soft_tolerance = 6_000.max((source as f64 * 0.02).round() as i64);
    let hard_tolerance = 12_000.max((source as f64 * 0.05).round() as i64);
    if difference > hard_tolerance {
        return DurationEvidence {
            score: 0.0,
            difference_ms: Some(difference),
            incompatible: true,
            outside_soft_tolerance: true,
        };
    }
    if difference <= 2_000 {
        return DurationEvidence {
            score: 1.0,
            difference_ms: Some(difference),
            incompatible: false,
            outside_soft_tolerance: false,
        };
    }
    if difference <= soft_tolerance {
        let span = (soft_tolerance - 2_000).max(1) as f64;
        let score = 1.0 - (difference - 2_000) as f64 / span;
        return DurationEvidence {
            score,
            difference_ms: Some(difference),
            incompatible: false,
            outside_soft_tolerance: false,
        };
    }
    DurationEvidence {
        score: 0.0,
        difference_ms: Some(difference),
        incompatible: false,
        outside_soft_tolerance: true,
    }
}

fn text_similarity(left: &str, right: &str) -> f64 {
    if left.is_empty() || right.is_empty() {
        0.0
    } else if left == right {
        1.0
    } else {
        normalized_levenshtein(left, right)
    }
}

fn artist_similarity(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left == right {
        return 1.0;
    }
    let intersection = left.intersection(right).count() as f64;
    let union = left.union(right).count() as f64;
    intersection / union
}

fn track_disc_similarity(left: &MatchTrackDescriptor, right: &MatchTrackDescriptor) -> f64 {
    let mut comparisons = Vec::new();
    if let (Some(left), Some(right)) = (left.track_number, right.track_number) {
        comparisons.push(if left == right { 1.0 } else { 0.0 });
    }
    if let (Some(left), Some(right)) = (left.disc_number, right.disc_number) {
        comparisons.push(if left == right { 1.0 } else { 0.0 });
    }
    if comparisons.is_empty() {
        0.0
    } else {
        comparisons.iter().sum::<f64>() / comparisons.len() as f64
    }
}

fn weighted_score(similarity: f64, points: f64) -> i64 {
    (similarity.clamp(0.0, 1.0) * points * 100.0).round() as i64
}

fn normalized_isrc(value: Option<&str>) -> Option<String> {
    value
        .map(|value| {
            value
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .map(|character| character.to_ascii_uppercase())
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
}

fn title_trigrams(value: &str) -> BTreeSet<String> {
    let compact = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<Vec<_>>();
    if compact.len() < 3 {
        return [compact.iter().collect::<String>()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect();
    }
    compact
        .windows(3)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureTrack {
        id: i64,
        title: String,
        artists: Vec<String>,
        album: Option<String>,
        isrc: Option<String>,
        duration_ms: Option<i64>,
        disc_number: Option<i64>,
        track_number: Option<i64>,
        explicit: Option<bool>,
        version_kind: Option<String>,
        version_detail: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MatcherFixture {
        name: String,
        source: FixtureTrack,
        candidates: Vec<FixtureTrack>,
        expected: MatchOutcome,
        selected_library_track_id: Option<i64>,
    }

    impl From<FixtureTrack> for MatchTrackDescriptor {
        fn from(value: FixtureTrack) -> Self {
            Self {
                id: value.id,
                title: value.title,
                artists: value.artists,
                album: value.album,
                isrc: value.isrc,
                duration_ms: value.duration_ms,
                disc_number: value.disc_number,
                track_number: value.track_number,
                explicit: value.explicit,
                version_kind: value.version_kind,
                version_detail: value.version_detail,
            }
        }
    }

    #[test]
    fn representative_fixture_corpus_matches_expected_outcomes() {
        let fixtures: Vec<MatcherFixture> =
            serde_json::from_str(include_str!("../tests/fixtures/matcher_cases.json"))
                .expect("matcher fixtures should parse");
        assert!(
            fixtures.len() >= 12,
            "fixture corpus should remain representative"
        );
        for fixture in fixtures {
            let source = MatchTrackDescriptor::from(fixture.source);
            let candidates = fixture
                .candidates
                .into_iter()
                .map(MatchTrackDescriptor::from)
                .collect();
            let result = MatcherIndex::new(candidates).match_track(&source, None, &HashSet::new());
            assert_eq!(
                result.outcome, fixture.expected,
                "fixture: {}",
                fixture.name
            );
            assert_eq!(
                result.selected_library_track_id, fixture.selected_library_track_id,
                "fixture: {}",
                fixture.name
            );
        }
    }

    #[test]
    fn normalization_uses_nfkc_punctuation_and_ampersand_equivalence() {
        assert_eq!(
            normalize_comparison_text("  Ａrtist & Guest—Mix  "),
            "artist and guest mix"
        );
    }

    #[test]
    fn featured_artists_are_compared_as_artist_information() {
        let source = MatchTrackDescriptor {
            id: 1,
            title: "Song (feat. Guest)".into(),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            isrc: None,
            duration_ms: Some(180_000),
            disc_number: Some(1),
            track_number: Some(1),
            explicit: None,
            version_kind: None,
            version_detail: None,
        };
        let candidate = MatchTrackDescriptor {
            id: 2,
            title: "Song".into(),
            artists: vec!["Artist".into(), "Guest".into()],
            ..source.clone()
        };
        let result = MatcherIndex::new(vec![candidate]).match_track(&source, None, &HashSet::new());
        assert_eq!(result.outcome, MatchOutcome::Automatic);
    }

    #[test]
    fn persisted_link_short_circuits_candidate_scoring() {
        let source = basic_track(1, "Song");
        let library = basic_track(2, "Different");
        let link = PersistedTrackLink {
            library_track_id: 2,
            method: "user".into(),
            confidence: 10_000,
            confirmed_by_user: true,
        };
        let result =
            MatcherIndex::new(vec![library]).match_track(&source, Some(&link), &HashSet::new());
        assert_eq!(result.outcome, MatchOutcome::Automatic);
        assert_eq!(result.selected_library_track_id, Some(2));
        assert_eq!(result.method.as_deref(), Some("user"));
    }

    #[test]
    fn duplicate_compatible_isrc_candidates_require_review() {
        let mut source = basic_track(1, "Song");
        source.isrc = Some("US-AAA-0000001".into());
        let mut first = basic_track(2, "Song");
        first.isrc = Some("USAAA0000001".into());
        let mut second = basic_track(3, "Song");
        second.isrc = Some("USAAA0000001".into());

        let result =
            MatcherIndex::new(vec![first, second]).match_track(&source, None, &HashSet::new());
        assert_eq!(result.outcome, MatchOutcome::Review);
        assert_eq!(result.selected_library_track_id, None);
    }

    #[test]
    fn incompatible_duplicate_isrc_does_not_block_unique_valid_match() {
        let mut source = basic_track(1, "Song");
        source.isrc = Some("USAAA0000001".into());
        let mut valid = basic_track(2, "Song");
        valid.isrc = Some("USAAA0000001".into());
        let mut incompatible = basic_track(3, "Song (Live)");
        incompatible.isrc = Some("USAAA0000001".into());

        let result = MatcherIndex::new(vec![valid, incompatible]).match_track(
            &source,
            None,
            &HashSet::new(),
        );
        assert_eq!(result.outcome, MatchOutcome::Automatic);
        assert_eq!(result.selected_library_track_id, Some(2));
        assert_eq!(result.method.as_deref(), Some("isrc"));
    }

    #[test]
    fn rejected_candidate_is_never_selected() {
        let source = basic_track(1, "Song");
        let library = basic_track(2, "Song");
        let result =
            MatcherIndex::new(vec![library]).match_track(&source, None, &HashSet::from([2]));
        assert_eq!(result.outcome, MatchOutcome::Unresolved);
        assert_eq!(
            result.candidates[0].incompatibilities,
            vec!["userRejected".to_owned()]
        );
    }

    #[test]
    fn fuzzy_fallback_uses_bounded_title_candidates() {
        let source = basic_track(1, "Signel");
        let library = basic_track(2, "Signal");
        let result = MatcherIndex::new(vec![library]).match_track(&source, None, &HashSet::new());
        assert!(!result.candidates.is_empty());
        assert_eq!(result.candidates[0].library_track_id, 2);
    }

    fn basic_track(id: i64, title: &str) -> MatchTrackDescriptor {
        MatchTrackDescriptor {
            id,
            title: title.into(),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            isrc: None,
            duration_ms: Some(180_000),
            disc_number: Some(1),
            track_number: Some(1),
            explicit: None,
            version_kind: None,
            version_detail: None,
        }
    }
}
