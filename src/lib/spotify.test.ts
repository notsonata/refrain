import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  cancelSpotifySourceRefresh,
  connectSpotify,
  disconnectSpotify,
  getSpotifyAuthStatus,
  refreshSpotifySource,
  spotifyErrorMessage,
  type SpotifyAuthStatus,
  type SpotifySourceRefreshSummary,
} from './spotify';

const status: SpotifyAuthStatus = {
  clientId: 'client-id',
  connected: true,
  accessTokenCached: true,
  registeredRedirectUri: 'http://127.0.0.1:43817/callback',
};

const summary: SpotifySourceRefreshSummary = {
  accountDisplayName: 'Listener',
  likedSongs: 20,
  playlists: 3,
  refreshedPlaylists: 2,
  unchangedPlaylists: 1,
  inaccessiblePlaylists: 0,
  removedPlaylists: 0,
  syncedAt: 1_790_000_000_000,
};

describe('Spotify commands', () => {
  it('loads authentication status', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('get_spotify_auth_status');
      return status as T;
    }) as InvokeFn;

    await expect(getSpotifyAuthStatus(invoke)).resolves.toEqual(status);
  });

  it('connects with the supplied Client ID', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        expect(command).toBe('connect_spotify');
        expect(args).toEqual({ clientId: 'client-id' });
        return status as T;
      },
    ) as InvokeFn;

    await expect(connectSpotify('client-id', invoke)).resolves.toEqual(status);
  });

  it('disconnects through the backend', async () => {
    const disconnected = { ...status, connected: false };
    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('disconnect_spotify');
      return disconnected as T;
    }) as InvokeFn;

    await expect(disconnectSpotify(invoke)).resolves.toEqual(disconnected);
  });

  it('refreshes Spotify source state', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('refresh_spotify_source');
      return summary as T;
    }) as InvokeFn;

    await expect(refreshSpotifySource(invoke)).resolves.toEqual(summary);
  });

  it('cancels an active source refresh', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('cancel_spotify_source_refresh');
      return true as T;
    }) as InvokeFn;

    await expect(cancelSpotifySourceRefresh(invoke)).resolves.toBe(true);
  });

  it('uses structured Spotify error messages', () => {
    expect(
      spotifyErrorMessage({
        code: 'authorizationCancelled',
        message: 'Spotify authorization was cancelled.',
      }),
    ).toBe('Spotify authorization was cancelled.');
  });
});
