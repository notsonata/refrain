import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';
import type { LocalFile } from './library';
import type { MatchCandidateEvidence, MatchOutcome } from './matching';

export type IssueKind =
  | 'matchReview'
  | 'missingLocalFile'
  | 'localOnlyTrack'
  | 'inaccessibleCollection'
  | 'invalidLocalFile'
  | 'acquisitionFailed';

export interface IssueRow {
  id: string;
  kind: IssueKind;
  title: string;
  subtitle: string | null;
  detail: string | null;
  sourceTrackId: number | null;
  libraryTrackId: number | null;
  localFileId: number | null;
  collectionId: number | null;
  candidateCount: number | null;
  confidence: number | null;
  path: string | null;
}

export interface IssueCounts {
  matchReview: number;
  missingLocalFile: number;
  localOnlyTrack: number;
  inaccessibleCollection: number;
  invalidLocalFile: number;
  acquisitionFailed: number;
}

export interface IssuePage {
  items: IssueRow[];
  total: number;
  offset: number;
  limit: number;
  counts: IssueCounts;
}

export interface MatchReviewTrack {
  id: number;
  title: string;
  artists: string[];
  album: string | null;
  isrc: string | null;
  durationMs: number | null;
  discNumber: number | null;
  trackNumber: number | null;
  explicit: boolean | null;
  versionKind: string | null;
  versionDetail: string | null;
}

export interface MatchReviewCandidate {
  track: MatchReviewTrack;
  evidence: MatchCandidateEvidence;
  files: LocalFile[];
}

export interface MatchReview {
  source: MatchReviewTrack;
  outcome: MatchOutcome;
  selectedLibraryTrackId: number | null;
  method: string | null;
  confidence: number | null;
  rejectedLibraryTrackIds: number[];
  candidates: MatchReviewCandidate[];
}

export function listIssues(
  offset: number,
  limit: number,
  invokeFn: InvokeFn = invoke,
): Promise<IssuePage> {
  return invokeFn<IssuePage>('list_issues', { offset, limit });
}

export function getMatchReview(
  sourceTrackId: number,
  invokeFn: InvokeFn = invoke,
): Promise<MatchReview> {
  return invokeFn<MatchReview>('get_match_review', { sourceTrackId });
}
