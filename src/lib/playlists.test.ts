import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  addTracksToLocalPlaylist,
  createLocalPlaylist,
  deleteLocalPlaylist,
  getLocalPlaylist,
  listLocalPlaylists,
  moveLocalPlaylistEntry,
  removeLocalPlaylistEntry,
  renameLocalPlaylist,
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
