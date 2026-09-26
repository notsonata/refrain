export type TrackStatus =
  'local-spotify' | 'local-only' | 'spotify-only' | 'needs-local-copy';

export const localLibraryTrackStatuses: readonly TrackStatus[] = [
  'local-spotify',
  'local-only',
];

export const spotifyLibraryTrackStatuses: readonly TrackStatus[] = [
  'local-spotify',
  'spotify-only',
  'needs-local-copy',
];

export function trackStatusLabel(status: TrackStatus): string {
  switch (status) {
    case 'local-spotify':
      return 'Local + Spotify';
    case 'local-only':
      return 'Local Only';
    case 'spotify-only':
      return 'Spotify Only';
    case 'needs-local-copy':
      return 'Needs Local Copy';
  }
}

export function localLibraryTrackStatus(onSpotify: boolean): TrackStatus {
  return onSpotify ? 'local-spotify' : 'local-only';
}

export function spotifyLibraryTrackStatus(
  localPresent: boolean,
  trackingIncluded: boolean,
): TrackStatus {
  if (localPresent) return 'local-spotify';
  return trackingIncluded ? 'needs-local-copy' : 'spotify-only';
}

export function trackStatusSortValue(status: TrackStatus): number {
  switch (status) {
    case 'local-spotify':
      return 0;
    case 'local-only':
      return 1;
    case 'spotify-only':
      return 2;
    case 'needs-local-copy':
      return 3;
  }
}
