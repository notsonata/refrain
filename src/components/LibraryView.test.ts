import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { LibraryTrackRow } from '../lib/library';
import LibraryView from './LibraryView.svelte';

const tracks: LibraryTrackRow[] = [
  {
    id: 1,
    title: 'Available Song',
    artists: ['Artist'],
    album: 'Album',
    releaseYear: 2026,
    durationMs: 180_000,
    sourceTrackCount: 2,
    localFileCount: 1,
    presentFileCount: 1,
    missingFileCount: 0,
    invalidFileCount: 0,
    preferredFile: {
      id: 10,
      path: '/music/Artist/Album/01 - Available Song.flac',
      ownership: 'external',
      state: 'present',
      format: 'flac',
    },
  },
  {
    id: 2,
    title: 'Missing Song',
    artists: ['Artist'],
    album: null,
    releaseYear: null,
    durationMs: null,
    sourceTrackCount: 1,
    localFileCount: 0,
    presentFileCount: 0,
    missingFileCount: 0,
    invalidFileCount: 0,
    preferredFile: null,
  },
];

describe('LibraryView', () => {
  it('renders logical tracks with available and missing state', () => {
    const { body } = render(LibraryView, {
      props: { tracks, total: tracks.length },
    });

    expect(body).toContain('Available Song');
    expect(body).toContain('Missing Song');
    expect(body).toContain('Available');
    expect(body).toContain('Missing');
    expect(body).toContain('01 - Available Song.flac');
  });
});
