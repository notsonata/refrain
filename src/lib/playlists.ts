import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface LocalPlaylistSummary {
  id: number;
  name: string;
  sourceCollectionId: number | null;
  imageUrl: string | null;
  customArtworkPath: string | null;
  entryCount: number;
  m3uPath: string | null;
  m3uManaged: boolean;
  lastSyncedAt: number | null;
  syncError: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface LocalPlaylistEntry {
  id: number;
  position: number;
  libraryTrackId: number;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  filePath: string | null;
  artworkPath: string | null;
}

export interface LocalPlaylistDetail {
  id: number;
  name: string;
  sourceCollectionId: number | null;
  imageUrl: string | null;
  customArtworkPath: string | null;
  m3uPath: string | null;
  m3uManaged: boolean;
  lastSyncedAt: number | null;
  syncError: string | null;
  createdAt: number;
  updatedAt: number;
  entries: LocalPlaylistEntry[];
}

// Older, independently imported M3Us may share a name with an automatically
// tracked Spotify mirror. Keep both records and files; collapse only the
// imported cards in the default browser, with an explicit way to reveal them.
export function localPlaylistBrowserItems(
  playlists: LocalPlaylistSummary[],
  showImportedNamesakes = false,
): LocalPlaylistSummary[] {
  if (showImportedNamesakes) return playlists;
  const mirrorNames = new Set(
    playlists
      .filter((playlist) => playlist.sourceCollectionId !== null)
      .map((playlist) => playlist.name.trim().toLocaleLowerCase()),
  );
  return playlists.filter(
    (playlist) =>
      playlist.sourceCollectionId !== null ||
      playlist.m3uPath === null ||
      playlist.m3uManaged ||
      !mirrorNames.has(playlist.name.trim().toLocaleLowerCase()),
  );
}

export function listLocalPlaylists(
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistSummary[]> {
  return invokeFn<LocalPlaylistSummary[]>('list_local_playlists');
}

export function getLocalPlaylist(
  playlistId: number,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('get_local_playlist', { playlistId });
}

export function createLocalPlaylist(
  name: string,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('create_local_playlist', { name });
}

export function renameLocalPlaylist(
  playlistId: number,
  name: string,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('rename_local_playlist', {
    playlistId,
    name,
  });
}

export function setLocalPlaylistArtwork(
  playlistId: number,
  path: string | null,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('set_local_playlist_artwork', {
    playlistId,
    path,
  });
}

export function deleteLocalPlaylist(
  playlistId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('delete_local_playlist', { playlistId });
}

export function setLocalPlaylistM3uPath(
  playlistId: number,
  path: string | null,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('set_local_playlist_m3u_path', {
    playlistId,
    path,
  });
}

export function addTracksToLocalPlaylist(
  playlistId: number,
  libraryTrackIds: number[],
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('add_tracks_to_local_playlist', {
    playlistId,
    libraryTrackIds,
  });
}

export function removeLocalPlaylistEntry(
  playlistId: number,
  entryId: number,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('remove_local_playlist_entry', {
    playlistId,
    entryId,
  });
}

export function moveLocalPlaylistEntry(
  playlistId: number,
  entryId: number,
  newPosition: number,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('move_local_playlist_entry', {
    playlistId,
    entryId,
    newPosition,
  });
}

export function syncLocalPlaylist(
  playlistId: number,
  invokeFn: InvokeFn = invoke,
): Promise<LocalPlaylistDetail> {
  return invokeFn<LocalPlaylistDetail>('sync_local_playlist', { playlistId });
}
