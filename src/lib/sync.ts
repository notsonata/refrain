import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export type SyncTrigger = 'manual' | 'startup' | 'scheduled';
export type SyncScope = 'legacy' | 'local' | 'spotify';
export type SyncStatus =
  'running' | 'succeeded' | 'partial' | 'failed' | 'cancelled';

export interface SyncRun {
  id: number;
  scope: SyncScope;
  trigger: SyncTrigger;
  status: SyncStatus;
  phase: string | null;
  startedAt: number;
  finishedAt: number | null;
  sourceAdded: number;
  sourceRemoved: number;
  matched: number;
  missing: number;
  needsReview: number;
  acquisitionFailed: number;
  errorMessage: string | null;
}

export interface SyncRunPage {
  items: SyncRun[];
  total: number;
  offset: number;
  limit: number;
}

export function startSync(
  trigger: SyncTrigger = 'manual',
  invokeFn: InvokeFn = invoke,
): Promise<SyncRun> {
  return invokeFn<SyncRun>('start_sync', { trigger });
}

export function startLocalSync(
  trigger: SyncTrigger = 'manual',
  invokeFn: InvokeFn = invoke,
): Promise<SyncRun> {
  return invokeFn<SyncRun>('start_local_sync', { trigger });
}

export function startSpotifySync(
  trigger: SyncTrigger = 'manual',
  invokeFn: InvokeFn = invoke,
): Promise<SyncRun> {
  return invokeFn<SyncRun>('start_spotify_sync', { trigger });
}

export function cancelSync(invokeFn: InvokeFn = invoke): Promise<boolean> {
  return invokeFn<boolean>('cancel_sync');
}

export function getSyncRun(
  runId: number,
  invokeFn: InvokeFn = invoke,
): Promise<SyncRun | null> {
  return invokeFn<SyncRun | null>('get_sync_run', { runId });
}

export function listSyncRuns(
  scope: SyncScope | null = null,
  offset = 0,
  limit = 20,
  invokeFn: InvokeFn = invoke,
): Promise<SyncRunPage> {
  return invokeFn<SyncRunPage>('list_sync_runs', { scope, offset, limit });
}
