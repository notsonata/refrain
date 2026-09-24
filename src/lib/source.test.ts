import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  formatTrackDuration,
  getSourceCollectionPage,
  hydrateSpotifySource,
  listSpotifySavedAlbums,
  type SourceCollectionListPage,
  type SourceCollectionPage,
  type SpotifySourceOverview,
} from './source';

const overview: SpotifySourceOverview = {
  account: {
    displayName: 'Listener',
    lastSourceSyncAt: 1_790_000_000_000,
  },
  likedSongs: {
    id: 1,
    providerCollectionId: 'spotify:liked-songs',
    kind: 'liked_songs',
    name: 'Liked Songs',
    isAccessible: true,
    accessIssue: null,
    entryCount: 249,
  },
  playlistCount: 55,
};

const playlists: SourceCollectionListPage = {
  items: [
    {
      id: 2,
      providerCollectionId: 'playlist',
      kind: 'playlist',
      name: 'Playlist',
      isAccessible: true,
      accessIssue: null,
      entryCount: 2,
    },
  ],
  total: 1,
  offset: 0,
  limit: 100,
};

const savedAlbums: SourceCollectionListPage = {
  items: [
    {
      id: 3,
      providerCollectionId: 'spotify:album:album',
      kind: 'saved_album',
      name: 'Saved Album',
      isAccessible: true,
      accessIssue: null,
      entryCount: 10,
    },
  ],
  total: 1,
  offset: 0,
  limit: 100,
};

const collection: SourceCollectionPage = {
  collection: playlists.items[0],
  entries: [
    {
      position: 0,
      itemType: 'track',
      addedAt: null,
      unavailableReason: null,
      track: {
        providerTrackId: 'track',
        title: 'Track',
        artists: ['Artist'],
        album: 'Album',
        durationMs: 181_000,
        explicit: false,
        imageUrl: null,
        externalUrl: null,
      },
    },
    {
      position: 1,
      itemType: 'track',
      addedAt: null,
      unavailableReason: null,
      track: {
        providerTrackId: 'track',
        title: 'Track',
        artists: ['Artist'],
        album: 'Album',
        durationMs: 181_000,
        explicit: false,
        imageUrl: null,
        externalUrl: null,
      },
    },
  ],
  total: 2,
  offset: 0,
  limit: 200,
};

describe('Spotify source browsing commands', () => {
  it('hydrates persisted source state through mocked Tauri IPC', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      if (command === 'get_spotify_source_overview') {
        return overview as T;
      }
      if (command === 'list_spotify_playlists') {
        return playlists as T;
      }
      if (command === 'list_spotify_saved_albums') {
        return savedAlbums as T;
      }
      throw new Error(`unexpected command: ${command}`);
    }) as InvokeFn;

    await expect(hydrateSpotifySource(invoke)).resolves.toEqual({
      overview,
      playlists,
      savedAlbums,
    });
    expect(invoke).toHaveBeenCalledWith('list_spotify_playlists', {
      offset: 0,
      limit: 100,
    });
    expect(invoke).toHaveBeenCalledWith('list_spotify_saved_albums', {
      offset: 0,
      limit: 100,
    });
  });

  it('lists saved albums through the dedicated browse command', async () => {
    const invoke = vi.fn(async <T>(command: string, args?: Record<string, unknown>) => {
      expect(command).toBe('list_spotify_saved_albums');
      expect(args).toEqual({ offset: 100, limit: 50 });
      return savedAlbums as T;
    }) as InvokeFn;

    await expect(listSpotifySavedAlbums(100, 50, invoke)).resolves.toEqual(savedAlbums);
  });

  it('loads ordered collection pages without collapsing duplicate positions', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('get_source_collection_page');
        expect(args).toEqual({ collectionId: 2, offset: 0, limit: 200 });
        return collection as T;
      },
    ) as InvokeFn;

    const page = await getSourceCollectionPage(2, 0, 200, invoke);
    expect(page.entries).toHaveLength(2);
    expect(page.entries.map((entry) => entry.position)).toEqual([0, 1]);
    expect(page.entries[0].track?.providerTrackId).toBe(
      page.entries[1].track?.providerTrackId,
    );
  });

  it('formats durations for dense track rows', () => {
    expect(formatTrackDuration(181_000)).toBe('3:01');
    expect(formatTrackDuration(null)).toBe('—');
  });
});
