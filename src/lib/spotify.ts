import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface SpotifyAuthStatus {
  clientId: string | null;
  connected: boolean;
  accessTokenCached: boolean;
  registeredRedirectUri: string;
}

interface SpotifyAuthError {
  code?: unknown;
  message?: unknown;
}

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

export function spotifyErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null) {
    const authError = error as SpotifyAuthError;
    if (typeof authError.message === 'string') {
      return authError.message;
    }
  }

  return error instanceof Error ? error.message : String(error);
}
