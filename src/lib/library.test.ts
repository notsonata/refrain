import { describe, expect, it, vi } from 'vitest';
import type { InvokeFn } from './app-info';
import {
  getLocalLibraryOverview,
  hashLocalFile,
  listLibraryTracks,
  listLocalFiles,
  scanLocalLibrary,
  setPreferredLocalFile,
  type LocalFilePage,
  type LibraryTrackPage,
  type LocalLibraryOverview,
  type LocalLibraryScanSummary,
} from './library';

const overview: LocalLibraryOverview = {
  total: 3,
  present: 2,
  missing: 1,
  invalid: 0,
};

const page: LocalFilePage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 100,
};

const libraryTrackPage: LibraryTrackPage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 100,
};

const summary: LocalLibraryScanSummary = {
  discovered: 2,
  added: 1,
  updated: 0,
  unchanged: 1,
  moved: 0,
  missing: 0,
  invalid: 0,
};

describe('local library commands', () => {
  it('loads overview, logical tracks, and paginated files', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        if (command === 'get_local_library_overview') return overview as T;
        if (command === 'list_library_tracks') {
          expect(args).toEqual({ offset: 0, limit: 100 });
          return libraryTrackPage as T;
        }
        if (command === 'list_local_files') {
          expect(args).toEqual({ offset: 0, limit: 100 });
          return page as T;
        }
        throw new Error(`unexpected command: ${command}`);
      },
    ) as InvokeFn;

    await expect(getLocalLibraryOverview(invoke)).resolves.toEqual(overview);
    await expect(listLibraryTracks(0, 100, invoke)).resolves.toEqual(
      libraryTrackPage,
    );
    await expect(listLocalFiles(0, 100, invoke)).resolves.toEqual(page);
  });

  it('starts a scan through the dedicated command', async () => {
    const invoke = vi.fn(async <T>(command: string) => {
      expect(command).toBe('scan_local_library');
      return summary as T;
    }) as InvokeFn;

    await expect(scanLocalLibrary(invoke)).resolves.toEqual(summary);
  });

  it('exposes lazy hashing and preferred-file commands', async () => {
    const invoke = vi.fn(
      async <T>(command: string, args?: Record<string, unknown>) => {
        if (command === 'hash_local_file') {
          expect(args).toEqual({ localFileId: 7 });
          return 'abc123' as T;
        }
        if (command === 'set_preferred_local_file') {
          expect(args).toEqual({ localFileId: 7 });
          return undefined as T;
        }
        throw new Error(`unexpected command: ${command}`);
      },
    ) as InvokeFn;

    await expect(hashLocalFile(7, invoke)).resolves.toBe('abc123');
    await expect(setPreferredLocalFile(7, invoke)).resolves.toBeUndefined();
  });
});
