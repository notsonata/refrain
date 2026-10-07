import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type {
  LocalPlaylistDetail,
  LocalPlaylistSummary,
} from '../lib/playlists';
import LocalPlaylistsView from './LocalPlaylistsView.svelte';

const playlists: LocalPlaylistSummary[] = [
  {
    id: 1,
    name: 'Night Drive',
    sourceCollectionId: null,
    imageUrl: null,
    entryCount: 2,
    m3uPath: '/music/playlists/night-drive.m3u8',
    m3uManaged: true,
    lastSyncedAt: 1_800_000_000_000,
    syncError: null,
    createdAt: 1_799_000_000_000,
    updatedAt: 1_800_000_000_000,
  },
];

const selectedPlaylist: LocalPlaylistDetail = {
  ...playlists[0],
  entries: [
    {
      id: 10,
      position: 0,
      libraryTrackId: 100,
      title: 'First Song',
      artists: ['Artist'],
      album: 'Album',
      durationMs: 180_000,
      filePath: '/music/Artist/Album/01 - First Song.flac',
      artworkPath: null,
    },
    {
      id: 11,
      position: 1,
      libraryTrackId: 101,
      title: 'Second Song',
      artists: ['Artist'],
      album: 'Album',
      durationMs: 200_000,
      filePath: '/music/Artist/Album/02 - Second Song.flac',
      artworkPath: null,
    },
  ],
};

describe('LocalPlaylistsView', () => {
  it('renders playlist detail as an automatic managed M3U workspace', () => {
    const { body } = render(LocalPlaylistsView, {
      props: { playlists, selectedPlaylist },
    });

    expect(body).toContain('Back to local playlists');
    expect(body).toContain('<strong>Local</strong>');
    expect(body).toContain('<span>Playlists</span>');
    expect(body).toContain('Night Drive');
    expect(body).toContain('M3U8');
    expect(body).toContain('night-drive.m3u8');
    expect(body).toContain('First Song');
    expect(body).toContain('Second Song');
    expect(body).toContain('Synced');
    expect(body).not.toContain('Choose File');
    expect(body).not.toContain('M3U Sync');
  });

  it('shows a searchable playlist browser before a playlist is opened', () => {
    const { body } = render(LocalPlaylistsView, {
      props: { playlists, selectedPlaylist: null },
    });

    expect(body).toContain('aria-label="Search local playlists"');
    expect(body).toContain('Night Drive');
    expect(body).not.toContain('Select a playlist');
  });

  it('explains how to populate an empty playlist', () => {
    const { body } = render(LocalPlaylistsView, {
      props: {
        playlists,
        selectedPlaylist: { ...selectedPlaylist, entries: [] },
      },
    });

    expect(body).toContain('Local > Songs');
    expect(body).toMatch(/add them to this\s+playlist/);
  });

  it('labels Spotify mirrors without duplicating mirror behavior copy', () => {
    const mirrored = {
      ...selectedPlaylist,
      sourceCollectionId: 44,
      name: 'Late Nights',
      imageUrl: 'https://i.scdn.co/image/late-nights-cover',
    };
    const { body } = render(LocalPlaylistsView, {
      props: {
        playlists: [
          {
            ...playlists[0],
            id: 2,
            name: 'Late Nights',
            sourceCollectionId: 44,
            imageUrl: 'https://i.scdn.co/image/late-nights-cover',
          },
        ],
        selectedPlaylist: mirrored,
      },
    });

    expect(body).toContain('Spotify mirror');
    expect(body).not.toContain(
      'Updates automatically from the tracked Spotify collection.',
    );

    const { body: browserBody } = render(LocalPlaylistsView, {
      props: {
        playlists: [
          {
            ...playlists[0],
            id: 2,
            name: 'Late Nights',
            sourceCollectionId: 44,
            imageUrl: 'https://i.scdn.co/image/late-nights-cover',
          },
        ],
        selectedPlaylist: null,
      },
    });
    expect(browserBody).toContain('https://i.scdn.co/image/late-nights-cover');
  });
});
