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
});
