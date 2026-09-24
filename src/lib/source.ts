import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './app-info';

export interface SourceAccountOverview {
  displayName: string | null;
  lastSourceSyncAt: number | null;
}

export interface SourceCollectionSummary {
  id: number;
  providerCollectionId: string;
  kind: string;
  name: string;
  isAccessible: boolean;
  accessIssue: string | null;
  entryCount: number;
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
  providerTrackId: string;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  explicit: boolean | null;
  imageUrl: string | null;
  externalUrl: string | null;
}

export interface SourceCollectionEntryView {
  position: number;
  itemType: string;
  addedAt: number | null;
  unavailableReason: string | null;
  track: SourceTrackView | null;
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
