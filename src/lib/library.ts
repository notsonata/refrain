import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface LocalFile {
  id: number;
  libraryTrackId: number | null;
  path: string;
  ownership: 'managed' | 'external';
  isPreferred: boolean;
  state: 'present' | 'missing' | 'invalid';
  format: string | null;
  fileSize: number;
  modifiedAt: number;
  durationMs: number | null;
  bitrate: number | null;
  sampleRate: number | null;
  channels: number | null;
  contentHash: string | null;
  tagTitle: string | null;
  tagArtists: string[];
  tagAlbum: string | null;
  tagIsrc: string | null;
  artworkPath: string | null;
  artworkMime: string | null;
  scanError: string | null;
}

export interface LocalFilePage {
  items: LocalFile[];
  total: number;
  offset: number;
  limit: number;
}

export interface LocalLibraryOverview {
  total: number;
  present: number;
  missing: number;
  invalid: number;
}

export interface LibraryTrackFileSummary {
  id: number;
  path: string;
  ownership: 'managed' | 'external';
  state: 'present' | 'missing' | 'invalid';
  format: string | null;
  artworkPath: string | null;
  artworkMime: string | null;
}

export interface SpotifyMembership {
  kind: 'liked_songs' | 'saved_album' | 'playlist' | string;
  name: string;
}

export interface LibraryTrackRow {
  id: number;
  title: string;
  artists: string[];
  album: string | null;
  releaseYear: number | null;
  durationMs: number | null;
  explicit: boolean | null;
  sourceTrackCount: number;
  acquisitionStatus: string | null;
  localFileCount: number;
  presentFileCount: number;
  missingFileCount: number;
  invalidFileCount: number;
  preferredFile: LibraryTrackFileSummary | null;
  spotifyMemberships: SpotifyMembership[];
}

export interface LibraryTrackPage {
  items: LibraryTrackRow[];
  total: number;
  offset: number;
  limit: number;
}

export interface LocalLibraryScanProgress {
  phase: string;
  completed: number;
  total: number | null;
  message: string;
}

export interface LocalLibraryScanSummary {
  discovered: number;
  added: number;
  updated: number;
  unchanged: number;
  moved: number;
  missing: number;
  invalid: number;
}

export function getLocalLibraryOverview(
  invokeFn: InvokeFn = invoke,
): Promise<LocalLibraryOverview> {
  return invokeFn<LocalLibraryOverview>('get_local_library_overview');
}

export function listLibraryTracks(
  offset: number,
  limit: number,
  invokeFn: InvokeFn = invoke,
): Promise<LibraryTrackPage> {
  return invokeFn<LibraryTrackPage>('list_library_tracks', { offset, limit });
}

export function listLocalFiles(
  offset: number,
  limit: number,
  invokeFn: InvokeFn = invoke,
): Promise<LocalFilePage> {
  return invokeFn<LocalFilePage>('list_local_files', { offset, limit });
}

export function scanLocalLibrary(
  invokeFn: InvokeFn = invoke,
): Promise<LocalLibraryScanSummary> {
  return invokeFn<LocalLibraryScanSummary>('scan_local_library');
}

export function hashLocalFile(
  localFileId: number,
  invokeFn: InvokeFn = invoke,
): Promise<string> {
  return invokeFn<string>('hash_local_file', { localFileId });
}

export function setPreferredLocalFile(
  localFileId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('set_preferred_local_file', { localFileId });
}
