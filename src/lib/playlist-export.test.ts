import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  exportPlaylist,
  listPlaylistExports,
  playlistExportErrorMessage,
} from './playlist-export';

describe('playlist export commands', () => {
  it('uses the immutable source-playlist export command surface', async () => {
    const calls: Array<[string, Record<string, unknown> | undefined]> = [];
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        calls.push([command, args]);
        return undefined as T;
      },
    ) as InvokeFn;

    await exportPlaylist(42, 'm3u8', '/exports/road-trip.m3u8', invoke);
    await exportPlaylist(42, 'bundle', '/exports/road-trip.zip', invoke);
    await listPlaylistExports(42, 0, 10, invoke);

    expect(calls).toEqual([
      [
        'export_playlist',
        {
          collectionId: 42,
          mode: 'm3u8',
          destination: '/exports/road-trip.m3u8',
        },
      ],
      [
        'export_playlist',
        {
          collectionId: 42,
          mode: 'bundle',
          destination: '/exports/road-trip.zip',
        },
      ],
      ['list_playlist_exports', { collectionId: 42, offset: 0, limit: 10 }],
    ]);
  });

  it('reads structured backend export errors', () => {
    expect(
      playlistExportErrorMessage({
        code: 'UNRESOLVED_ENTRIES',
        message: 'Cannot export because 2 playlist entries are unresolved.',
        unresolvedCount: 2,
      }),
    ).toBe('Cannot export because 2 playlist entries are unresolved.');
  });
});
