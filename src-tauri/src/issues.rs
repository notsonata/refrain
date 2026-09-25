use std::collections::HashMap;

use crate::{
    db::{Database, DatabaseError},
    domain::{
        IssueCounts, IssueKind, IssuePage, IssueRow, MatchOutcome, MatchReview,
        MatchReviewCandidate, MatchReviewTrack, MatchTrackDescriptor,
    },
    matching::MatcherIndex,
};

const MAX_ISSUE_PAGE_LIMIT: u32 = 500;

pub(crate) fn list_issues(
    database: &Database,
    offset: u32,
    limit: u32,
) -> Result<IssuePage, DatabaseError> {
    let limit = limit.clamp(1, MAX_ISSUE_PAGE_LIMIT);
    let mut issues = match_review_issues(database)?;
    issues.extend(database.non_match_issue_rows()?);
    issues.sort_by(|left, right| {
        issue_priority(left.kind)
            .cmp(&issue_priority(right.kind))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });

    let counts = issue_counts(&issues);
    let total = issues.len();
    let start = usize::try_from(offset).unwrap_or(usize::MAX).min(total);
    let end = start
        .saturating_add(usize::try_from(limit).unwrap_or(usize::MAX))
        .min(total);
    Ok(IssuePage {
        items: issues[start..end].to_vec(),
        total,
        offset,
        limit,
        counts,
    })
}

pub(crate) fn get_match_review(
    database: &Database,
    source_track_id: i64,
) -> Result<Option<MatchReview>, DatabaseError> {
    let Some(source) = database.source_match_track(source_track_id)? else {
        return Ok(None);
    };
    let library_tracks = database.library_match_tracks()?;
    let existing_link = database.persisted_track_link(source_track_id)?;
    let rejected = database.rejected_library_track_ids(source_track_id)?;
    let result = MatcherIndex::new(library_tracks.clone()).match_track(
        &source,
        existing_link.as_ref(),
        &rejected,
    );
    let tracks_by_id = library_tracks
        .into_iter()
        .map(|track| (track.id, track))
        .collect::<HashMap<_, _>>();
    let mut candidates = Vec::with_capacity(result.candidates.len());
    for evidence in &result.candidates {
        let track = tracks_by_id
            .get(&evidence.library_track_id)
            .ok_or_else(|| {
                DatabaseError::InvalidState(format!(
                    "match candidate {} is missing from the library projection",
                    evidence.library_track_id
                ))
            })?;
        candidates.push(MatchReviewCandidate {
            track: review_track(track),
            evidence: evidence.clone(),
            files: database.local_files_for_library_track(track.id)?,
        });
    }

    Ok(Some(MatchReview {
        source: review_track(&source),
        outcome: result.outcome,
        selected_library_track_id: result.selected_library_track_id,
        method: result.method,
        confidence: result.confidence,
        rejected_library_track_ids: result.rejected_library_track_ids,
        candidates,
    }))
}

fn match_review_issues(database: &Database) -> Result<Vec<IssueRow>, DatabaseError> {
    let library_tracks = database.library_match_tracks()?;
    let matcher = MatcherIndex::new(library_tracks);
    let mut issues = Vec::new();
    for source_track_id in database.accessible_source_track_ids()? {
        let Some(source) = database.source_match_track(source_track_id)? else {
            continue;
        };
        let existing_link = database.persisted_track_link(source_track_id)?;
        let rejected = database.rejected_library_track_ids(source_track_id)?;
        let result = matcher.match_track(&source, existing_link.as_ref(), &rejected);
        if result.outcome != MatchOutcome::Review {
            continue;
        }
        issues.push(IssueRow {
            id: format!("match:{source_track_id}"),
            kind: IssueKind::MatchReview,
            title: source.title,
            subtitle: (!source.artists.is_empty()).then(|| source.artists.join(", ")),
            detail: source
                .album
                .map(|album| format!("Ambiguous match from {album}. Review local candidates."))
                .or_else(|| Some("Review the candidate local tracks before linking.".into())),
            source_track_id: Some(source_track_id),
            library_track_id: None,
            local_file_id: None,
            collection_id: None,
            candidate_count: Some(result.candidates.len()),
            confidence: result.confidence,
            path: None,
        });
    }
    Ok(issues)
}

fn review_track(track: &MatchTrackDescriptor) -> MatchReviewTrack {
    MatchReviewTrack {
        id: track.id,
        title: track.title.clone(),
        artists: track.artists.clone(),
        album: track.album.clone(),
        isrc: track.isrc.clone(),
        duration_ms: track.duration_ms,
        disc_number: track.disc_number,
        track_number: track.track_number,
        explicit: track.explicit,
        version_kind: track.version_kind.clone(),
        version_detail: track.version_detail.clone(),
    }
}

fn issue_counts(issues: &[IssueRow]) -> IssueCounts {
    let mut counts = IssueCounts::default();
    for issue in issues {
        match issue.kind {
            IssueKind::MatchReview => counts.match_review += 1,
            IssueKind::MissingLocalFile => counts.missing_local_file += 1,
            IssueKind::InaccessibleCollection => counts.inaccessible_collection += 1,
            IssueKind::InvalidLocalFile => counts.invalid_local_file += 1,
            IssueKind::AcquisitionFailed => counts.acquisition_failed += 1,
        }
    }
    counts
}

fn issue_priority(kind: IssueKind) -> u8 {
    match kind {
        IssueKind::MatchReview => 0,
        IssueKind::MissingLocalFile => 1,
        IssueKind::InvalidLocalFile => 2,
        IssueKind::AcquisitionFailed => 3,
        IssueKind::InaccessibleCollection => 4,
    }
}
