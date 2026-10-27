import { describe, expect, it } from 'vitest';
import {
  mirrorProgressDetails,
  mirrorProgressSummary,
  mirrorTimeEstimate,
  updateMirrorTiming,
  type MirrorProgress,
} from './mirror';

const progress: MirrorProgress = {
  targetId: 1,
  phase: 'working',
  operation: 'Copying file',
  currentFile: 'Music/Artist/Album/song.flac',
  completed: 9,
  total: 80,
  copied: 3,
  unchanged: 6,
  removed: 0,
  fileBytesDone: 5 * 1024 * 1024,
  fileBytesTotal: 10 * 1024 * 1024,
};

describe('mirror operation feedback', () => {
  it('shows the current operation, item count and filename in the sidebar', () => {
    expect(mirrorProgressSummary(progress)).toBe(
      'Copying file · 9/80\nsong.flac',
    );
  });

  it('shows the current relative path, transfer bytes and action totals in details', () => {
    expect(mirrorProgressDetails(progress)).toContain(
      'Music/Artist/Album/song.flac',
    );
    expect(mirrorProgressDetails(progress)).toContain('5.0 MB / 10.0 MB');
    expect(mirrorProgressDetails(progress)).toContain(
      '3 copied · 6 unchanged · 0 removed',
    );
  });

  it('shows hashing byte progress while keeping copy counts distinct', () => {
    const hashing = {
      ...progress,
      phase: 'scanning' as const,
      operation: 'Hashing source file',
    };
    expect(mirrorProgressDetails(hashing)).toContain('5.0 MB / 10.0 MB hashed');
  });

  it('labels portable playlist output as an export', () => {
    const playlist = {
      ...progress,
      operation: 'Exporting playlist',
      currentFile: 'Playlists/Mix-1.m3u8',
    };
    expect(mirrorProgressSummary(playlist)).toContain('Mix-1.m3u8');
    expect(mirrorProgressDetails(playlist)).toContain('exported for this file');
  });
});

describe('mirror time estimates', () => {
  it('tracks total elapsed time across stages while using the current stage for ETA', () => {
    const hashing = {
      ...progress,
      operation: 'Hashing source file',
      fileBytesDone: 0,
    };
    const started = updateMirrorTiming(
      null,
      { ...hashing, phase: 'preparing', completed: 0, total: 0 },
      1000,
    );
    const scanning = updateMirrorTiming(
      started,
      { ...hashing, phase: 'scanning', completed: 0, total: 10 },
      6000,
    );
    expect(
      mirrorTimeEstimate(
        scanning,
        { ...hashing, phase: 'scanning', completed: 0, total: 10 },
        6000,
      ),
    ).toBe('Elapsed 5s · Stage ETA calculating…');

    const next = updateMirrorTiming(
      scanning,
      { ...hashing, phase: 'scanning', completed: 2, total: 10 },
      12000,
    );
    expect(
      mirrorTimeEstimate(
        next,
        { ...hashing, phase: 'scanning', completed: 2, total: 10 },
        12000,
      ),
    ).toBe('Elapsed 11s · Stage ~24s left');
  });

  it('starts per-file ETA when copying begins and resets for the next file', () => {
    const comparing = {
      ...progress,
      completed: 0,
      total: 2,
      operation: 'Comparing file',
      fileBytesDone: 0,
    };
    const copying = {
      ...comparing,
      operation: 'Copying file',
      fileBytesDone: 2 * 1024 * 1024,
    };
    const initial = updateMirrorTiming(null, comparing, 1000);
    const transfer = updateMirrorTiming(initial, copying, 5000);
    expect(mirrorTimeEstimate(transfer, copying, 5000)).not.toContain('File ~');
    expect(mirrorTimeEstimate(transfer, copying, 7000)).toContain(
      'File ~8s left',
    );

    const nextFile = { ...copying, currentFile: 'Music/next.flac' };
    const restarted = updateMirrorTiming(transfer, nextFile, 8000);
    expect(mirrorTimeEstimate(restarted, nextFile, 8000)).not.toContain(
      'File ~',
    );
    expect(mirrorTimeEstimate(restarted, nextFile, 10000)).toContain(
      'File ~8s left',
    );
  });

  it('shows file ETA while hashing and resets when changing operations', () => {
    const hashing = {
      ...progress,
      phase: 'scanning' as const,
      operation: 'Hashing source file',
      completed: 0,
      total: 3,
    };
    const initial = updateMirrorTiming(null, hashing, 1000);
    expect(mirrorTimeEstimate(initial, hashing, 5000)).toContain(
      'File ~4s left',
    );
    const checking = { ...hashing, operation: 'Checking destination' };
    const changed = updateMirrorTiming(initial, checking, 6000);
    expect(mirrorTimeEstimate(changed, checking, 6000)).not.toContain('File ~');
    expect(mirrorTimeEstimate(changed, checking, 8000)).toContain(
      'File ~2s left',
    );
  });

  it('does not divide by zero or invent an ETA before measurable progress', () => {
    const waiting = {
      ...progress,
      operation: 'Reading mirror state',
      fileBytesDone: 0,
    };
    const initial = updateMirrorTiming(
      null,
      { ...waiting, completed: 0, total: 0 },
      0,
    );
    expect(
      mirrorTimeEstimate(
        initial,
        { ...waiting, completed: 0, total: 0 },
        10000,
      ),
    ).toBe('Elapsed 10s · Stage ETA calculating…');
    expect(
      mirrorTimeEstimate(
        initial,
        { ...waiting, completed: 0, total: 4 },
        10000,
      ),
    ).toContain('Stage ETA calculating…');
  });
});
