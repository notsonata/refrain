import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  connectSpotify,
  disconnectSpotify,
  getSpotifyAuthStatus,
  spotifyErrorMessage,
  type SpotifyAuthStatus,
} from './spotify';

const status: SpotifyAuthStatus = {
  clientId: 'client-id',
  connected: true,
  accessTokenCached: true,
  registeredRedirectUri: 'http://127.0.0.1/callback',
};

describe('Spotify auth commands', () => {
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

  it('uses structured authentication error messages', () => {
    expect(
      spotifyErrorMessage({
        code: 'authorizationCancelled',
        message: 'Spotify authorization was cancelled.',
      }),
    ).toBe('Spotify authorization was cancelled.');
  });
});
