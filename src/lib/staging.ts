import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';
import type { AcquisitionCandidate } from './acquisition';

export type StagingStage =
  | 'needsLocalCopy'
  | 'queued'
  | 'searching'
  | 'downloading'
  | 'needsResolution'
  | 'failed'
  | 'downloaded'
  | 'cancelled';

export interface StagingItem {
  libraryTrackId: number;
  jobId: number | null;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  imageUrl: string | null;
  provider: string | null;
  providerJobId: string | null;
  stage: StagingStage;
  jobStatus: 'queued' | 'running' | 'staged' | 'failed' | 'cancelled' | null;
  attempt: number;
  candidate: AcquisitionCandidate | null;
  candidates: AcquisitionCandidate[];
  stagingPath: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  bytesTransferred: number | null;
  totalBytes: number | null;
  createdAt: number | null;
  startedAt: number | null;
  finishedAt: number | null;
  updatedAt: number | null;
}

export interface StagingPage {
  items: StagingItem[];
  total: number;
  offset: number;
  limit: number;
}

export interface ContinueStagingItemResult {
  jobId: number;
  libraryTrackId: number | null;
  imported: boolean;
  errorCode: string | null;
  errorMessage: string | null;
}

export interface ContinueStagingSummary {
  imported: number;
  failed: number;
  results: ContinueStagingItemResult[];
}

export function listStagingItems(
  offset: number,
  limit: number,
  invokeFn: InvokeFn = invoke,
): Promise<StagingPage> {
  return invokeFn<StagingPage>('list_staging_items', { offset, limit });
}

export function startStagingTrack(
  libraryTrackId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('start_staging_track', { libraryTrackId });
}

export function cancelStagingTrack(
  jobId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('cancel_staging_track', { jobId });
}

export function resolveStagingTrack(
  jobId: number,
  provider: string,
  providerToken: string,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('resolve_staging_track', {
    jobId,
    provider,
    providerToken,
  });
}

export function rejectStagingCandidate(
  jobId: number,
  provider: string,
  providerToken: string,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('reject_staging_candidate', {
    jobId,
    provider,
    providerToken,
  });
}

export function searchAgainStagingTrack(
  jobId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('search_again_staging_track', { jobId });
}

export function startAllStaging(invokeFn: InvokeFn = invoke): Promise<void> {
  return invokeFn<void>('start_all_staging');
}

export function cancelActiveStaging(
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('cancel_active_staging');
}

export function resetAcquisitionSession(
  invokeFn: InvokeFn = invoke,
): Promise<number> {
  return invokeFn<number>('reset_acquisition_session');
}

export function retryFailedStaging(invokeFn: InvokeFn = invoke): Promise<void> {
  return invokeFn<void>('retry_failed_staging');
}

export function continueStagingTracks(
  jobIds: number[],
  invokeFn: InvokeFn = invoke,
): Promise<ContinueStagingSummary> {
  return invokeFn<ContinueStagingSummary>('continue_staging_tracks', {
    jobIds,
  });
}

export function excludeStagingTrackFromTracking(
  libraryTrackId: number,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('exclude_staging_track_from_tracking', {
    libraryTrackId,
  });
}
