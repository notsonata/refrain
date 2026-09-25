import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface AcquisitionCandidate {
  providerToken: string;
  title: string | null;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  format: string | null;
  sizeBytes: number | null;
}

export interface AcquisitionJob {
  id: number;
  libraryTrackId: number;
  trackTitle: string;
  trackArtists: string[];
  provider: string;
  providerJobId: string | null;
  status: 'queued' | 'running' | 'staged' | 'failed' | 'cancelled';
  attempt: number;
  candidate: AcquisitionCandidate | null;
  stagingPath: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  updatedAt: number;
}

export interface AcquisitionJobPage {
  items: AcquisitionJob[];
  total: number;
  offset: number;
  limit: number;
}

export function listAcquisitionJobs(
  offset: number,
  limit: number,
  invokeFn: InvokeFn = invoke,
): Promise<AcquisitionJobPage> {
  return invokeFn<AcquisitionJobPage>('list_acquisition_jobs', {
    offset,
    limit,
  });
}
