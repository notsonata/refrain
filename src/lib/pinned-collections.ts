import type { SourceCollectionSummary } from './source';

export type PinnedSpotifyCollectionKind = 'saved_album' | 'playlist';

export interface PinnedSpotifyCollection {
  id: number;
  providerCollectionId: string;
  kind: PinnedSpotifyCollectionKind;
  name: string;
  imageUrl: string | null;
}

const storageKey = 'refrain-spotify-pinned-collections';

export function pinnedCollectionKey(collection: {
  kind: string;
  providerCollectionId: string;
}): string {
  return `${collection.kind}:${collection.providerCollectionId}`;
}

export function isPinnableSpotifyCollection(
  collection: SourceCollectionSummary,
): collection is SourceCollectionSummary & {
  kind: PinnedSpotifyCollectionKind;
} {
  return collection.kind === 'saved_album' || collection.kind === 'playlist';
}

export function toPinnedSpotifyCollection(
  collection: SourceCollectionSummary,
): PinnedSpotifyCollection | null {
  if (!isPinnableSpotifyCollection(collection)) return null;
  return {
    id: collection.id,
    providerCollectionId: collection.providerCollectionId,
    kind: collection.kind,
    name: collection.name,
    imageUrl: collection.imageUrl,
  };
}

export function togglePinnedSpotifyCollection(
  pins: PinnedSpotifyCollection[],
  collection: SourceCollectionSummary,
): PinnedSpotifyCollection[] {
  const pin = toPinnedSpotifyCollection(collection);
  if (!pin) return pins;

  const key = pinnedCollectionKey(pin);
  if (pins.some((item) => pinnedCollectionKey(item) === key)) {
    return pins.filter((item) => pinnedCollectionKey(item) !== key);
  }
  return [...pins, pin];
}

export function sortPinnedSpotifyCollectionsFirst(
  collections: SourceCollectionSummary[],
  pinnedKeys: string[],
): SourceCollectionSummary[] {
  if (pinnedKeys.length === 0) return collections;

  const pinOrder = new Map(pinnedKeys.map((key, index) => [key, index]));
  return collections
    .map((collection, index) => ({ collection, index }))
    .sort((left, right) => {
      const leftPin = pinOrder.get(pinnedCollectionKey(left.collection));
      const rightPin = pinOrder.get(pinnedCollectionKey(right.collection));

      if (leftPin !== undefined && rightPin !== undefined) {
        return leftPin - rightPin;
      }
      if (leftPin !== undefined) return -1;
      if (rightPin !== undefined) return 1;
      return left.index - right.index;
    })
    .map(({ collection }) => collection);
}

export function loadPinnedSpotifyCollections(): PinnedSpotifyCollection[] {
  if (typeof window === 'undefined') return [];
  try {
    const raw = window.localStorage.getItem(storageKey);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isPinnedRecord);
  } catch {
    return [];
  }
}

export function savePinnedSpotifyCollections(
  pins: PinnedSpotifyCollection[],
): void {
  if (typeof window === 'undefined') return;
  window.localStorage.setItem(storageKey, JSON.stringify(pins));
}

function isPinnedRecord(value: unknown): value is PinnedSpotifyCollection {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.id === 'number' &&
    typeof record.providerCollectionId === 'string' &&
    (record.kind === 'saved_album' || record.kind === 'playlist') &&
    typeof record.name === 'string' &&
    (record.imageUrl === null || typeof record.imageUrl === 'string')
  );
}
