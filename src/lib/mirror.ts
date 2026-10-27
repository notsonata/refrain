import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface MirrorRun {
  id: number;
  targetId: number;
  status: 'running' | 'succeeded' | 'failed' | 'cancelled';
  startedAt: number;
  finishedAt: number | null;
  copied: number;
  unchanged: number;
  removed: number;
  error: string | null;
}

export interface MirrorTarget {
  id: number;
  path: string;
  identity: string;
  createdAt: number;
  updatedAt: number;
  lastRun: MirrorRun | null;
  available: boolean;
}

export interface MirrorProgress {
  targetId: number;
  completed: number;
  total: number;
  phase:
    | 'preparing'
    | 'scanning'
    | 'playlists'
    | 'verifying'
    | 'working'
    | 'copying'
    | 'finalizing'
    | 'complete';
  operation: string;
  currentFile: string | null;
  copied: number;
  unchanged: number;
  removed: number;
  fileBytesDone: number;
  fileBytesTotal: number;
}

export interface MirrorTiming {
  stage: string;
  runStartedAt: number;
  stageStartedAt: number;
  initialCompleted: number;
  file: string | null;
  fileOperation: string;
  fileStartedAt: number;
}

export interface MirrorTransferRate {
  file: string;
  bytesDone: number;
  sampledAt: number;
  bytesPerSecond: number | null;
}

export function updateMirrorTransferRate(
  previous: MirrorTransferRate | null,
  progress: MirrorProgress,
  now: number,
): MirrorTransferRate | null {
  if (progress.operation !== 'Copying file' || !progress.currentFile) {
    return null;
  }
  if (
    !previous ||
    previous.file !== progress.currentFile ||
    progress.fileBytesDone < previous.bytesDone
  ) {
    return {
      file: progress.currentFile,
      bytesDone: progress.fileBytesDone,
      sampledAt: now,
      bytesPerSecond: null,
    };
  }
  const elapsedMs = now - previous.sampledAt;
  const bytesCopied = progress.fileBytesDone - previous.bytesDone;
  if (bytesCopied <= 0 || elapsedMs <= 0) return previous;

  const instantRate = (bytesCopied * 1000) / elapsedMs;
  return {
    ...previous,
    bytesDone: progress.fileBytesDone,
    sampledAt: now,
    bytesPerSecond:
      previous.bytesPerSecond === null
        ? instantRate
        : previous.bytesPerSecond * 0.65 + instantRate * 0.35,
  };
}

// 'working' operations and file completion events belong to one copy stage.
const mirrorStage = (phase: MirrorProgress['phase']) =>
  phase === 'working' ? 'copying' : phase;

export function updateMirrorTiming(
  previous: MirrorTiming | null,
  progress: MirrorProgress,
  now: number,
): MirrorTiming {
  const stage = mirrorStage(progress.phase);
  if (
    !previous ||
    previous.stage !== stage ||
    progress.completed < previous.initialCompleted
  ) {
    return {
      stage,
      runStartedAt: previous?.runStartedAt ?? now,
      stageStartedAt: now,
      initialCompleted: progress.completed,
      file: progress.currentFile,
      fileOperation: progress.operation,
      fileStartedAt: now,
    };
  }
  return {
    ...previous,
    file: progress.currentFile,
    fileOperation: progress.operation,
    fileStartedAt:
      previous.file === progress.currentFile &&
      previous.fileOperation === progress.operation
        ? previous.fileStartedAt
        : now,
  };
}

const duration = (seconds: number) => {
  const rounded = Math.max(0, Math.ceil(seconds));
  if (rounded < 60) return `${rounded}s`;
  const minutes = Math.floor(rounded / 60);
  if (minutes < 60) return `${minutes}m ${rounded % 60}s`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
};

export function mirrorTimeEstimate(
  timing: MirrorTiming | null,
  progress: MirrorProgress | null,
  now: number,
): string {
  if (!timing || !progress) return 'Estimating time…';
  const elapsed = Math.max(0, (now - timing.runStartedAt) / 1000);
  const stageElapsed = Math.max(0, (now - timing.stageStartedAt) / 1000);
  if (timing.stage === 'complete') return 'Complete';
  const completedSinceStart = progress.completed - timing.initialCompleted;
  const remaining = progress.total - progress.completed;
  const stageEstimate =
    progress.total > 0 &&
    remaining > 0 &&
    completedSinceStart > 0 &&
    stageElapsed >= 2
      ? `Stage ~${duration((stageElapsed / completedSinceStart) * remaining)} left`
      : progress.total > 0 && remaining === 0
        ? 'Stage complete'
        : 'Stage ETA calculating…';
  let fileEstimate = '';
  const fileElapsed = (now - timing.fileStartedAt) / 1000;
  if (
    progress.fileBytesTotal > progress.fileBytesDone &&
    progress.fileBytesDone > 0 &&
    fileElapsed >= 2
  ) {
    fileEstimate = ` · File ~${duration((fileElapsed * (progress.fileBytesTotal - progress.fileBytesDone)) / progress.fileBytesDone)} left`;
  }
  return `Elapsed ${duration(elapsed)} · ${stageEstimate}${fileEstimate}`;
}

const fileSize = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
};

export function mirrorTransferSpeed(
  transfer: MirrorTransferRate | null,
  progress: MirrorProgress | null,
  now: number,
): string | null {
  if (
    !transfer ||
    !progress ||
    progress.operation !== 'Copying file' ||
    progress.currentFile !== transfer.file ||
    transfer.bytesPerSecond === null
  ) {
    return null;
  }
  if (now - transfer.sampledAt > 2500) return '0 B/s';
  return `${fileSize(Math.round(transfer.bytesPerSecond))}/s`;
}

export function mirrorProgressSummary(progress: MirrorProgress): string {
  const count =
    progress.total > 0 ? ` · ${progress.completed}/${progress.total}` : '';
  const name = progress.currentFile?.split('/').at(-1);
  return `${progress.operation}${count}${name ? `\n${name}` : ''}`;
}

export function mirrorProgressDetails(progress: MirrorProgress): string {
  const count =
    progress.total > 0
      ? ` · ${progress.completed}/${progress.total} items`
      : '';
  const bytes =
    progress.fileBytesTotal > 0
      ? `\n${fileSize(progress.fileBytesDone)} / ${fileSize(progress.fileBytesTotal)} ${progress.operation === 'Copying file' ? 'copied' : progress.operation === 'Exporting playlist' ? 'exported' : progress.operation === 'Hashing source file' ? 'hashed' : 'checked'} for this file`
      : '';
  return `${progress.operation}${count}${progress.currentFile ? `\n${progress.currentFile}` : ''}${bytes}\n${progress.copied} copied · ${progress.unchanged} unchanged · ${progress.removed} removed`;
}

export const listMirrorTargets = (invokeFn: InvokeFn = invoke) =>
  invokeFn<MirrorTarget[]>('list_mirror_targets');

export const saveMirrorTarget = (path: string, invokeFn: InvokeFn = invoke) =>
  invokeFn<MirrorTarget>('save_mirror_target', { path });

export const removeMirrorTarget = (id: number, invokeFn: InvokeFn = invoke) =>
  invokeFn<void>('remove_mirror_target', { id });

export const runMirror = (targetId: number, invokeFn: InvokeFn = invoke) =>
  invokeFn<MirrorRun>('run_mirror', { targetId });

export const cancelMirror = (targetId: number, invokeFn: InvokeFn = invoke) =>
  invokeFn<boolean>('cancel_mirror', { targetId });

export const listMirrorRuns = (targetId: number, invokeFn: InvokeFn = invoke) =>
  invokeFn<MirrorRun[]>('list_mirror_runs', { targetId });
