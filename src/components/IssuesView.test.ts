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
  it('renders all Milestone 11 issue states', () => {
    const { body } = render(IssuesView, {
      props: {
        issues,
        total: issues.length,
        counts: {
          matchReview: 1,
          missingLocalFile: 1,
          invalidLocalFile: 1,
          inaccessibleCollection: 1,
        },
      },
    });

    expect(body).toContain('Needs review');
    expect(body).toContain('Missing');
    expect(body).toContain('Invalid file');
    expect(body).toContain('Inaccessible');
  });

  it('renders candidate evidence and clearable rejection decisions', () => {
    const { body } = render(IssuesView, {
      props: {
        issues: [issues[0]],
        total: 1,
        counts: {
          matchReview: 1,
          missingLocalFile: 0,
          invalidLocalFile: 0,
          inaccessibleCollection: 0,
        },
        selectedIssueId: 'match:7',
        review,
      },
    });

    expect(body).toContain('Spotify source');
    expect(body).toContain('Candidate Song');
    expect(body).toContain('runner-up is close');
    expect(body).toContain('Clear rejection');
    expect(body).not.toContain('Confirm match');
  });
});
