import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface SourceAccountOverview {
  displayName: string | null;
  imageUrl: string | null;
  lastSourceSyncAt: number | null;
}

export interface SourceAlbumMetadata {
  artists: string[];
  releaseDate: string | null;
  albumType: string | null;
  label: string | null;
  copyrights: string[];
  externalUrl: string | null;
}

export interface SourceCollectionSummary {
  id: number;
  providerCollectionId: string;
  kind: string;
  name: string;
  isAccessible: boolean;
  accessIssue: string | null;
  entryCount: number;
  trackedByDefault: boolean;
  trackedEntryCount: number;
  localEntryCount: number;
  attentionEntryCount: number;
  imageUrl: string | null;
  externalUrl?: string | null;
  albumMetadata?: SourceAlbumMetadata | null;
}

export interface SpotifySourceOverview {
  account: SourceAccountOverview | null;
  likedSongs: SourceCollectionSummary | null;
  playlistCount: number;
}

export interface SourceCollectionListPage {
  items: SourceCollectionSummary[];
  total: number;
  offset: number;
  limit: number;
}

export interface SourceTrackView {
  id: number;
  providerTrackId: string;
  title: string;
  artists: string[];
  album: string | null;
  releaseYear: number | null;
  durationMs: number | null;
  explicit: boolean | null;
  imageUrl: string | null;
  externalUrl: string | null;
  localPresent: boolean;
  localFormat: string | null;
  matchState: 'matched' | 'unmatched' | string;
  acquisitionStatus: string | null;
}

export interface SourceCollectionEntryView {
  position: number;
  itemType: string;
  addedAt: number | null;
  unavailableReason: string | null;
  track: SourceTrackView | null;
  trackingIncluded: boolean;
  trackingOverridden: boolean;
}

export interface SourceCollectionPage {
  collection: SourceCollectionSummary;
  entries: SourceCollectionEntryView[];
  total: number;
  offset: number;
  limit: number;
}

export interface SpotifySourceHydration {
  overview: SpotifySourceOverview;
  playlists: SourceCollectionListPage;
  savedAlbums: SourceCollectionListPage;
}

export function getSpotifySourceOverview(
  invokeFn: InvokeFn = invoke,
): Promise<SpotifySourceOverview> {
  return invokeFn<SpotifySourceOverview>('get_spotify_source_overview');
}

export function listSpotifyPlaylists(
  offset = 0,
  limit = 100,
  invokeFn: InvokeFn = invoke,
): Promise<SourceCollectionListPage> {
  return invokeFn<SourceCollectionListPage>('list_spotify_playlists', {
    offset,
    limit,
  });
}

export function listSpotifySavedAlbums(
  offset = 0,
  limit = 100,
  invokeFn: InvokeFn = invoke,
): Promise<SourceCollectionListPage> {
  return invokeFn<SourceCollectionListPage>('list_spotify_saved_albums', {
    offset,
    limit,
  });
}

export function getSourceCollectionPage(
  collectionId: number,
  offset = 0,
  limit = 200,
  invokeFn: InvokeFn = invoke,
): Promise<SourceCollectionPage> {
  return invokeFn<SourceCollectionPage>('get_source_collection_page', {
    collectionId,
    offset,
    limit,
  });
}

export function setSourceCollectionTracking(
  collectionId: number,
  included: boolean,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('set_source_collection_tracking', {
    collectionId,
    included,
  });
}

export function setSourceTrackTracking(
  collectionId: number,
  sourceTrackId: number,
  included: boolean | null,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('set_source_track_tracking', {
    collectionId,
    sourceTrackId,
    included,
  });
}

export function setSourceTracksTracking(
  collectionId: number,
  sourceTrackIds: number[],
  included: boolean | null,
  invokeFn: InvokeFn = invoke,
): Promise<void> {
  return invokeFn<void>('set_source_tracks_tracking', {
    collectionId,
    sourceTrackIds,
    included,
  });
}

export async function hydrateSpotifySource(
  invokeFn: InvokeFn = invoke,
): Promise<SpotifySourceHydration> {
  const [overview, playlists, savedAlbums] = await Promise.all([
    getSpotifySourceOverview(invokeFn),
    listSpotifyPlaylists(0, 100, invokeFn),
    listSpotifySavedAlbums(0, 100, invokeFn),
  ]);
  return { overview, playlists, savedAlbums };
}

export function formatTrackDuration(durationMs: number | null): string {
  if (durationMs === null || durationMs < 0) {
    return '—';
  }

  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}
