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
    explicit: false,
    sourceTrackCount: 2,
    acquisitionStatus: null,
    localFileCount: 1,
    presentFileCount: 1,
    missingFileCount: 0,
    invalidFileCount: 0,
    spotifyMemberships: [
      { kind: 'liked_songs', name: 'Liked Songs' },
      { kind: 'playlist', name: 'Favorites' },
    ],
    preferredFile: {
      id: 10,
      path: '/music/Artist/Album/01 - Available Song.flac',
      ownership: 'external',
      state: 'present',
      format: 'flac',
      artworkPath: null,
      artworkMime: null,
    },
  },
  {
    id: 2,
    title: 'Local Song',
    artists: ['Artist'],
    album: 'Second Album',
    releaseYear: 2025,
    durationMs: 205_000,
    explicit: null,
    sourceTrackCount: 0,
    acquisitionStatus: null,
    localFileCount: 1,
    presentFileCount: 1,
    missingFileCount: 0,
    invalidFileCount: 0,
    spotifyMemberships: [],
    preferredFile: {
      id: 11,
      path: '/music/Artist/Second Album/02 - Local Song.mp3',
      ownership: 'external',
      state: 'present',
      format: 'mp3',
      artworkPath: null,
      artworkMime: null,
    },
  },
];

describe('LibraryView', () => {
  it('renders present local tracks with Spotify state and file paths', () => {
    const { body } = render(LibraryView, {
      props: { tracks, total: tracks.length },
    });

    expect(body).toContain('Available Song');
    expect(body).toContain('Local Song');
    expect(body).toContain('On Spotify');
    expect(body).toContain('Local only');
    expect(body).toContain('01 - Available Song.flac');
    expect(body).toContain('02 - Local Song.mp3');
  });
});
