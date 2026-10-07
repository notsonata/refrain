import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type {
  SourceCollectionSummary,
  SpotifySourceOverview,
} from '../lib/source';
import SpotifyWorkspace from './SpotifyWorkspace.svelte';

const overview: SpotifySourceOverview = {
  account: {
    displayName: 'Listener',
    imageUrl: null,
    lastSourceSyncAt: null,
  },
  likedSongs: {
    id: 1,
    providerCollectionId: 'spotify:liked-songs',
    kind: 'liked_songs',
    name: 'Liked Songs',
    isAccessible: true,
    accessIssue: null,
    entryCount: 249,
    trackedByDefault: true,
    trackedEntryCount: 249,
    localEntryCount: 105,
    attentionEntryCount: 144,
    imageUrl: null,
  },
  playlistCount: 0,
};

describe('SpotifyWorkspace liked songs', () => {
  it('distinguishes Spotify-only tracks from tracks that need a local copy', () => {
    const { body } = render(SpotifyWorkspace, {
      props: {
        section: 'liked',
        overview,
        entries: [],
        collectionTotal: 249,
      },
    });

    expect(body).toContain('Spotify Only');
    expect(body).toContain('Needs Local Copy');
    expect(body).toContain('Track Liked Songs');
    expect(body).toContain('Liked Songs selection actions');
    expect(body).not.toContain('Need Attention');
  });

  it('separates selection actions from album detail actions', () => {
    const album: SourceCollectionSummary = {
      id: 2,
      providerCollectionId: 'album-2',
      kind: 'saved_album',
      name: 'Example Album',
      isAccessible: true,
      accessIssue: null,
      entryCount: 8,
      trackedByDefault: false,
      trackedEntryCount: 0,
      localEntryCount: 0,
      attentionEntryCount: 0,
      imageUrl: 'https://example.com/cover.jpg',
      externalUrl: 'https://open.spotify.com/album/example',
      albumMetadata: {
        artists: ['Artist'],
        releaseDate: '2026-01-01',
        albumType: 'album',
        label: 'Label',
        copyrights: [],
        externalUrl: 'https://open.spotify.com/album/example',
      },
    };

    const { body } = render(SpotifyWorkspace, {
      props: {
        section: 'albums',
        overview,
        currentCollection: album,
        savedAlbums: [album],
        savedAlbumTotal: 1,
        entries: [],
        collectionTotal: 8,
      },
    });

    expect(body).toContain('Example Album selection actions');
    expect(body).toContain('Example Album details actions');
  });

  it('marks pinned collection cards and renders them first in their page', () => {
    const playlist: SourceCollectionSummary = {
      id: 3,
      providerCollectionId: 'playlist-3',
      kind: 'playlist',
      name: 'Pinned Mix',
      isAccessible: true,
      accessIssue: null,
      entryCount: 12,
      trackedByDefault: false,
      trackedEntryCount: 0,
      localEntryCount: 0,
      attentionEntryCount: 0,
      imageUrl: null,
    };
    const otherPlaylist: SourceCollectionSummary = {
      ...playlist,
      id: 4,
      providerCollectionId: 'playlist-4',
      name: 'Earlier Mix',
    };

    const { body } = render(SpotifyWorkspace, {
      props: {
        section: 'playlists',
        overview,
        playlists: [otherPlaylist, playlist],
        playlistTotal: 2,
        pinnedCollectionKeys: ['playlist:playlist-3'],
      },
    });

    expect(body).toContain('Pinned Mix');
    expect(body).toContain('aria-label="Pinned"');
    expect(body.indexOf('Pinned Mix')).toBeLessThan(
      body.indexOf('Earlier Mix'),
    );
  });

  it('shows immutable export actions for a Spotify playlist detail', () => {
    const playlist: SourceCollectionSummary = {
      id: 5,
      providerCollectionId: 'playlist-5',
      kind: 'playlist',
      name: 'Road Trip',
      isAccessible: true,
      accessIssue: null,
      entryCount: 3,
      trackedByDefault: true,
      trackedEntryCount: 3,
      localEntryCount: 3,
      attentionEntryCount: 0,
      imageUrl: null,
      externalUrl: 'https://open.spotify.com/playlist/road-trip',
    };

    const { body } = render(SpotifyWorkspace, {
      props: {
        section: 'playlists',
        overview,
        currentCollection: playlist,
        playlists: [playlist],
        playlistTotal: 1,
        entries: [],
        collectionTotal: 3,
      },
    });

    expect(body).toContain('Exports');
    expect(body).toContain('Immutable playlist snapshots');
    expect(body).toContain('M3U8');
    expect(body).toContain('Bundle');
    expect(body).toContain('No exports yet.');
  });
});
