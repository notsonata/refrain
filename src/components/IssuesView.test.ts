import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { IssueRow, MatchReview } from '../lib/issues';
import IssuesView from './IssuesView.svelte';

const issues: IssueRow[] = [
  {
    id: 'match:7',
    kind: 'matchReview',
    title: 'Ambiguous Song',
    subtitle: 'Artist',
    detail: 'Review local candidates.',
    sourceTrackId: 7,
    libraryTrackId: null,
    localFileId: null,
    collectionId: null,
    candidateCount: 1,
    confidence: 9_100,
    path: null,
    imageUrl: 'https://example.test/ambiguous.jpg',
    artworkPath: null,
  },
  {
    id: 'missing-library:8',
    kind: 'missingLocalFile',
    title: 'Missing Song',
    subtitle: 'Artist',
    detail: 'No present file.',
    sourceTrackId: null,
    libraryTrackId: 8,
    localFileId: null,
    collectionId: null,
    candidateCount: null,
    confidence: null,
    path: null,
    imageUrl: 'https://example.test/missing.jpg',
    artworkPath: null,
  },
  {
    id: 'invalid-file:9',
    kind: 'invalidLocalFile',
    title: 'Broken.flac',
    subtitle: 'Invalid local file',
    detail: 'Unreadable tags',
    sourceTrackId: null,
    libraryTrackId: null,
    localFileId: 9,
    collectionId: null,
    candidateCount: null,
    confidence: null,
    path: '/music/Broken.flac',
    imageUrl: null,
    artworkPath: null,
  },
  {
    id: 'collection:10',
    kind: 'inaccessibleCollection',
    title: 'Blocked Playlist',
    subtitle: 'Spotify playlist is inaccessible',
    detail: 'Spotify denied access',
    sourceTrackId: null,
    libraryTrackId: null,
    localFileId: null,
    collectionId: 10,
    candidateCount: null,
    confidence: null,
    path: null,
    imageUrl: 'https://example.test/collection.jpg',
    artworkPath: null,
  },
];

const review: MatchReview = {
  source: {
    id: 7,
    title: 'Ambiguous Song',
    artists: ['Artist'],
    album: 'Album',
    isrc: null,
    durationMs: 180_000,
    discNumber: 1,
    trackNumber: 1,
    explicit: false,
    versionKind: null,
    versionDetail: null,
    imageUrl: 'https://example.test/ambiguous.jpg',
  },
  outcome: 'review',
  selectedLibraryTrackId: null,
  method: null,
  confidence: 9_100,
  rejectedLibraryTrackIds: [11],
  candidates: [
    {
      track: {
        id: 11,
        title: 'Candidate Song',
        artists: ['Artist'],
        album: 'Album',
        isrc: null,
        durationMs: 180_100,
        discNumber: 1,
        trackNumber: 1,
        explicit: false,
        versionKind: null,
        versionDetail: null,
        imageUrl: 'https://example.test/candidate.jpg',
      },
      evidence: {
        libraryTrackId: 11,
        relationship: 'uncertain',
        score: {
          title: 3_000,
          artists: 2_500,
          duration: 2_000,
          album: 1_200,
          trackDisc: 400,
          total: 9_100,
        },
        exactIsrc: false,
        durationDifferenceMs: 100,
        warnings: ['runner-up is close'],
        incompatibilities: [],
      },
      files: [],
    },
  ],
};

describe('IssuesView', () => {
  it('renders genuine non-acquisition issue states', () => {
    const { body } = render(IssuesView, {
      props: {
        issues,
        total: issues.length,
        counts: {
          matchReview: 1,
          missingLocalFile: 1,
          localOnlyTrack: 0,
          invalidLocalFile: 1,
          inaccessibleCollection: 1,
          acquisitionFailed: 0,
        },
      },
    });

    expect(body).toContain('4 unresolved');
    expect(body).toContain('1 review');
    expect(body).toContain('1 missing');
    expect(body).toContain('1 inaccessible');
    expect(body).toContain('1 invalid');
    expect(body).not.toContain('Local Only');
  });

  it('renders candidate evidence and clearable rejection decisions', () => {
    const { body } = render(IssuesView, {
      props: {
        issues: [issues[0]],
        total: 1,
        counts: {
          matchReview: 1,
          missingLocalFile: 0,
          localOnlyTrack: 0,
          invalidLocalFile: 0,
          inaccessibleCollection: 0,
          acquisitionFailed: 0,
        },
        selectedIssueId: 'match:7',
        review,
      },
    });

    expect(body).toContain('Spotify source');
    expect(body).toContain('Candidate Song');
    expect(body).toContain('runner-up is close');
    expect(body).toContain('Clear Rejection');
    expect(body).not.toContain('Confirm Match');
  });

  it('uses the compact attention summary and warning styling', () => {
    const { body } = render(IssuesView, {
      props: {
        issues: [issues[1]],
        total: 1,
        counts: {
          matchReview: 0,
          missingLocalFile: 1,
          localOnlyTrack: 0,
          invalidLocalFile: 0,
          inaccessibleCollection: 0,
          acquisitionFailed: 0,
        },
      },
    });

    expect(body).toContain('class="issues-summary"');
    expect(body).not.toContain('metrics-strip');
    expect(body).toContain('aria-label="Filter and sort issues"');
    expect(body).toContain('class="chip warning">Missing</span>');
  });

  it('renders issue artwork and a Trash action for invalid files', () => {
    const { body } = render(IssuesView, {
      props: {
        issues: [issues[2]],
        total: 1,
        counts: {
          matchReview: 0,
          missingLocalFile: 0,
          localOnlyTrack: 0,
          invalidLocalFile: 1,
          inaccessibleCollection: 0,
          acquisitionFailed: 0,
        },
        selectedIssueId: 'invalid-file:9',
      },
    });

    expect(body).toContain('Move to Trash');
    expect(body).toContain('system Trash or Recycle Bin');
  });

  it('uses source artwork in the issue queue and match review source', () => {
    const { body } = render(IssuesView, {
      props: {
        issues: [issues[0]],
        total: 1,
        counts: {
          matchReview: 1,
          missingLocalFile: 0,
          localOnlyTrack: 0,
          invalidLocalFile: 0,
          inaccessibleCollection: 0,
          acquisitionFailed: 0,
        },
        selectedIssueId: 'match:7',
        review,
      },
    });

    expect(body).toContain('issue-row-artwork');
    expect(body).toContain('issue-source-artwork');
    expect(body).toContain('https://example.test/ambiguous.jpg');
    expect(body).toContain('https://example.test/candidate.jpg');
  });
});
