import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  addTracksToLocalPlaylist,
  createLocalPlaylist,
  deleteLocalPlaylist,
  getLocalPlaylist,
  listLocalPlaylists,
  localPlaylistBrowserItems,
  moveLocalPlaylistEntry,
  removeLocalPlaylistEntry,
  renameLocalPlaylist,
  setLocalPlaylistArtwork,
  setLocalPlaylistM3uPath,
  syncLocalPlaylist,
} from './playlists';

describe('local playlist commands', () => {
  it('uses the local playlist command surface', async () => {
    const calls: Array<[string, Record<string, unknown> | undefined]> = [];
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        calls.push([command, args]);
        return undefined as T;
      },
    ) as InvokeFn;

    await listLocalPlaylists(invoke);
    await getLocalPlaylist(7, invoke);
    await createLocalPlaylist('Road Trip', invoke);
    await renameLocalPlaylist(7, 'Night Drive', invoke);
    await setLocalPlaylistArtwork(7, '/images/cover.png', invoke);
    await setLocalPlaylistArtwork(7, null, invoke);
    await setLocalPlaylistM3uPath(7, '/music/night-drive.m3u8', invoke);
    await addTracksToLocalPlaylist(7, [10, 11], invoke);
    await moveLocalPlaylistEntry(7, 19, 0, invoke);
    await removeLocalPlaylistEntry(7, 19, invoke);
    await syncLocalPlaylist(7, invoke);
    await deleteLocalPlaylist(7, invoke);

    expect(calls).toEqual([
      ['list_local_playlists', undefined],
      ['get_local_playlist', { playlistId: 7 }],
      ['create_local_playlist', { name: 'Road Trip' }],
      ['rename_local_playlist', { playlistId: 7, name: 'Night Drive' }],
      [
        'set_local_playlist_artwork',
        { playlistId: 7, path: '/images/cover.png' },
      ],
      ['set_local_playlist_artwork', { playlistId: 7, path: null }],
      [
        'set_local_playlist_m3u_path',
        { playlistId: 7, path: '/music/night-drive.m3u8' },
      ],
      [
        'add_tracks_to_local_playlist',
        { playlistId: 7, libraryTrackIds: [10, 11] },
      ],
      [
        'move_local_playlist_entry',
        { playlistId: 7, entryId: 19, newPosition: 0 },
      ],
      ['remove_local_playlist_entry', { playlistId: 7, entryId: 19 }],
      ['sync_local_playlist', { playlistId: 7 }],
      ['delete_local_playlist', { playlistId: 7 }],
    ]);
  });
});

describe('local playlist browser duplicate handling', () => {
  it('shows Spotify mirrors once but keeps imported namesakes accessible on demand', () => {
    const item = (
      id: number,
      name: string,
      sourceCollectionId: number | null,
      m3uPath: string | null,
      m3uManaged: boolean,
    ) => ({
      id,
      name,
      sourceCollectionId,
      m3uPath,
      m3uManaged,
      imageUrl: null,
      customArtworkPath: null,
      entryCount: 0,
      lastSyncedAt: null,
      syncError: null,
      createdAt: 1,
      updatedAt: 1,
    });
    const playlists = [
      item(1, 'Feels', 95, '/Music/Playlists/Feels.m3u8', true),
      item(2, 'Feels', null, '/Music/Feels.m3u', false),
      item(3, 'Feels', null, null, true),
      item(4, 'Good Stuff', null, '/Music/Good Stuff.m3u', false),
    ];

    expect(localPlaylistBrowserItems(playlists).map(({ id }) => id)).toEqual([
      1, 3, 4,
    ]);
    expect(localPlaylistBrowserItems(playlists, true)).toEqual(playlists);
  });
});
