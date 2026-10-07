import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export type PlaylistExportMode = 'm3u8' | 'bundle';

export interface PlaylistExport {
  id: number;
  collectionId: number;
  collectionName: string;
  mode: PlaylistExportMode;
  destination: string;
  status: 'running' | 'succeeded' | 'failed';
  entryCount: number;
  createdAt: number;
  finishedAt: number | null;
  errorMessage: string | null;
}

export interface PlaylistExportPage {
  items: PlaylistExport[];
  total: number;
  offset: number;
  limit: number;
}

export interface PlaylistExportCommandError {
  code: string;
  message: string;
  unresolvedCount?: number | null;
}

export function exportPlaylist(
  collectionId: number,
  mode: PlaylistExportMode,
  destination: string,
  invokeFn: InvokeFn = invoke,
): Promise<PlaylistExport> {
  return invokeFn<PlaylistExport>('export_playlist', {
    collectionId,
    mode,
    destination,
  });
}

export function listPlaylistExports(
  collectionId: number | null,
  offset = 0,
  limit = 10,
  invokeFn: InvokeFn = invoke,
): Promise<PlaylistExportPage> {
  return invokeFn<PlaylistExportPage>('list_playlist_exports', {
    collectionId,
    offset,
    limit,
  });
}

export function playlistExportErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string' && message.trim()) return message;
  }
  return String(error);
}
