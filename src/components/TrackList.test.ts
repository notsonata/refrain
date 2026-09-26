import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { SourceCollectionEntryView } from '../lib/source';
import TrackList from './TrackList.svelte';

const duplicateEntries: SourceCollectionEntryView[] = [0, 1].map(
  (position) => ({
    position,
    itemType: 'track',
    addedAt: null,
    unavailableReason: null,
    trackingIncluded: false,
    trackingOverridden: false,
    track: {
      id: 42,
      providerTrackId: 'same-track',
      title: 'Repeated Track',
      artists: ['Artist'],
      album: 'Album',
      releaseYear: 2026,
      durationMs: 180_000,
      explicit: false,
      imageUrl: null,
      externalUrl: null,
      localPresent: true,
      localFormat: 'flac',
      matchState: 'matched',
      acquisitionStatus: null,
    },
  }),
);

describe('TrackList', () => {
  it('renders intentional duplicate playlist positions separately', () => {
    const { body } = render(TrackList, {
      props: { entries: duplicateEntries, total: 2 },
    });

    expect(body.match(/Repeated Track/g)).toHaveLength(2);
    expect(body).toContain('1');
    expect(body).toContain('2');
  });

  it('renders an empty state without manufacturing rows', () => {
    const { body } = render(TrackList, {
      props: {
        entries: [],
        total: 0,
        emptyMessage: 'Nothing imported yet.',
      },
    });

    expect(body).toContain('Nothing imported yet.');
    expect(body).not.toContain('Unavailable Spotify item');
  });

  it('keeps per-song tracking controls and row selection available', () => {
    const { body } = render(TrackList, {
      props: {
        entries: duplicateEntries,
        total: 2,
        trackingControls: true,
      },
    });

    expect(body.match(/type="checkbox"/g)).toHaveLength(2);
    expect(body).toContain('Select Repeated Track');
    expect(body).not.toContain('Select All Shown');
    expect(body).not.toContain('Use Default for Selected');
    expect(body).toContain('Excluded');
    expect(body).toContain('Tracking');
  });
});
