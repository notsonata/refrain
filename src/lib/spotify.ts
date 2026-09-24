import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface SpotifyAuthStatus {
  clientId: string | null;
  connected: boolean;
  accessTokenCached: boolean;
  registeredRedirectUri: string;
}

export interface SpotifySourceRefreshProgress {
  phase: string;
  completed: number;
  total: number | null;
  message: string;
}

export interface SpotifySourceRefreshSummary {
  accountDisplayName: string | null;
  likedSongs: number;
  playlists: number;
  refreshedPlaylists: number;
  unchangedPlaylists: number;
  inaccessiblePlaylists: number;
  removedPlaylists: number;
  syncedAt: number;
}

interface SpotifyError {
  code?: unknown;
  message?: unknown;
}

const UNKNOWN_SPOTIFY_ERROR =
  'Spotify operation failed. Try again. If the problem persists, reconnect Spotify.';

export function getSpotifyAuthStatus(
  invokeFn: InvokeFn = invoke,
): Promise<SpotifyAuthStatus> {
  return invokeFn<SpotifyAuthStatus>('get_spotify_auth_status');
}

export function connectSpotify(
  clientId: string,
  invokeFn: InvokeFn = invoke,
): Promise<SpotifyAuthStatus> {
  return invokeFn<SpotifyAuthStatus>('connect_spotify', { clientId });
}

export function disconnectSpotify(
  invokeFn: InvokeFn = invoke,
): Promise<SpotifyAuthStatus> {
  return invokeFn<SpotifyAuthStatus>('disconnect_spotify');
}

export function refreshSpotifySource(
  invokeFn: InvokeFn = invoke,
): Promise<SpotifySourceRefreshSummary> {
  return invokeFn<SpotifySourceRefreshSummary>('refresh_spotify_source');
}

export function cancelSpotifySourceRefresh(
  invokeFn: InvokeFn = invoke,
): Promise<boolean> {
  return invokeFn<boolean>('cancel_spotify_source_refresh');
}

export function spotifyErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null) {
    const spotifyError = error as SpotifyError;
    if (
      typeof spotifyError.message === 'string' &&
      spotifyError.message.trim().length > 0
    ) {
      return spotifyError.message;
    }
  }

  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  if (typeof error === 'string' && error.trim().length > 0) {
    return error;
  }

  return UNKNOWN_SPOTIFY_ERROR;
}
