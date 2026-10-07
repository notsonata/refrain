import { describe, expect, it } from 'vitest';
import type { SourceCollectionSummary } from './source';
import {
  pinnedCollectionKey,
  sortPinnedSpotifyCollectionsFirst,
  togglePinnedSpotifyCollection,
} from './pinned-collections';

function collection(
  kind: 'saved_album' | 'playlist',
  providerCollectionId: string,
  name: string,
): SourceCollectionSummary {
  return {
    id: providerCollectionId === 'one' ? 1 : 2,
    providerCollectionId,
    kind,
    name,
    isAccessible: true,
    accessIssue: null,
    entryCount: 10,
    trackedByDefault: false,
    trackedEntryCount: 0,
    localEntryCount: 0,
    attentionEntryCount: 0,
    imageUrl: null,
  };
}

describe('pinned Spotify collections', () => {
  it('uses provider identity so album and playlist pins stay distinct', () => {
    expect(
      pinnedCollectionKey({
        kind: 'saved_album',
        providerCollectionId: 'same',
      }),
    ).toBe('saved_album:same');
    expect(
      pinnedCollectionKey({ kind: 'playlist', providerCollectionId: 'same' }),
    ).toBe('playlist:same');
  });

  it('toggles a pinned collection without disturbing other pins', () => {
    const album = collection('saved_album', 'one', 'One');
    const playlist = collection('playlist', 'two', 'Two');

    const withAlbum = togglePinnedSpotifyCollection([], album);
    const withBoth = togglePinnedSpotifyCollection(withAlbum, playlist);
    const playlistOnly = togglePinnedSpotifyCollection(withBoth, album);

    expect(withBoth.map((item) => item.name)).toEqual(['One', 'Two']);
    expect(playlistOnly.map((item) => item.name)).toEqual(['Two']);
  });

  it('moves pinned collections to the front while preserving pin order', () => {
    const one = collection('playlist', 'one', 'One');
    const two = collection('playlist', 'two', 'Two');
    const three = collection('playlist', 'three', 'Three');

    expect(
      sortPinnedSpotifyCollectionsFirst(
        [one, two, three],
        ['playlist:three', 'playlist:two'],
      ).map((item) => item.name),
    ).toEqual(['Three', 'Two', 'One']);
  });
});
