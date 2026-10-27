import { describe, expect, it } from 'vitest';
import {
  createLocalTrackColumns,
  createSpotifyTrackColumns,
  mergeSharedColumns,
  migrateLegacyColumns,
  restoredColumnWidth,
  tableGridTemplate,
} from './table-columns';

describe('Local Library table columns', () => {
  it('keeps Status compact without absorbing unused table width', () => {
    const columns = createLocalTrackColumns();
    const status = columns.find((column) => column.id === 'status')!;
    const gridTracks = tableGridTemplate(columns).match(
      /\d+px|minmax\([^)]*\)/g,
    )!;

    expect(status.width).toBe(92);
    expect(status.minWidth).toBe(64);
    expect(gridTracks[columns.indexOf(status)]).toBe('92px');
    expect(
      tableGridTemplate(
        columns.map((column) =>
          column.id === 'status' ? { ...column, width: 64 } : column,
        ),
      ).match(/\d+px|minmax\([^)]*\)/g)?.[columns.indexOf(status)],
    ).toBe('64px');
  });

  it('keeps the Local checkbox column narrow and locked without changing Spotify Order', () => {
    const local = createLocalTrackColumns();
    const checkbox = local.find((column) => column.id === 'order')!;
    const spotifyOrder = createSpotifyTrackColumns().find(
      (column) => column.id === 'order',
    )!;

    expect(checkbox.width).toBe(22);
    expect(checkbox.minWidth).toBe(22);
    expect(checkbox.resizable).toBe(false);
    expect(tableGridTemplate(local).split(' ')[0]).toBe('22px');
    expect(restoredColumnWidth(checkbox, 54)).toBe(22);
    expect(spotifyOrder.width).toBe(54);
    expect(spotifyOrder.resizable).toBe(true);
  });

  it('lets the locked Playlists column shrink to a count-only chip', () => {
    const columns = createLocalTrackColumns();
    const playlists = columns.find((column) => column.id === 'playlists')!;
    const gridTracks = tableGridTemplate(columns).match(
      /\d+px|minmax\([^)]*\)/g,
    )!;

    expect(playlists.draggable).toBe(false);
    expect(playlists.minWidth).toBe(48);
    expect(gridTracks[columns.indexOf(playlists)]).toBe(
      `minmax(48px, ${playlists.width}fr)`,
    );
  });

  it('resets legacy v5/v6 Local Status widths without discarding other column preferences', () => {
    const status = createLocalTrackColumns().find(
      (column) => column.id === 'status',
    )!;

    for (const version of ['v5', 'v6']) {
      const saved = {
        version,
        columns: [
          { id: 'title', width: 302 },
          { id: 'order', width: 54 },
          { id: 'status', width: 184 },
          { id: 'playlists', width: 74 },
        ],
        sortId: 'album',
        sortDirection: 'desc' as const,
      };
      const migrated = {
        ...saved,
        columns: migrateLegacyColumns(saved.columns, ['status'])!,
      };
      expect(migrated.columns).toEqual([
        { id: 'title', width: 302 },
        { id: 'order', width: 54 },
        { id: 'status', width: undefined },
        { id: 'playlists', width: 74 },
      ]);
      expect(migrated.sortId).toBe('album');
      expect(migrated.sortDirection).toBe('desc');
      expect(restoredColumnWidth(status, migrated.columns[2].width)).toBe(92);
    }
    expect(restoredColumnWidth(status, 184)).toBe(184);
    expect(restoredColumnWidth(status, 48)).toBe(64);
    expect(restoredColumnWidth(status, undefined)).toBe(92);
    const spotifyStatus = createSpotifyTrackColumns().find(
      (column) => column.id === 'status',
    )!;
    expect(restoredColumnWidth(spotifyStatus, 164)).toBe(164);
  });

  it('preserves Spotify table sizing', () => {
    const columns = createSpotifyTrackColumns();
    const status = columns.find((column) => column.id === 'status')!;
    const tracking = columns.find((column) => column.id === 'tracking')!;
    const gridTracks = tableGridTemplate(columns).match(
      /\d+px|minmax\([^)]*\)/g,
    )!;

    expect(status.minWidth).toBe(108);
    expect(gridTracks[columns.indexOf(status)]).toBe('minmax(0px, 126fr)');
    expect(gridTracks[columns.indexOf(tracking)]).toBe('112px');
  });

  it('retains the Spotify Status position and width when Local columns are saved', () => {
    const previous = [
      { id: 'title', width: 300 },
      { id: 'status', width: 175 },
      { id: 'artist', width: 140 },
    ];
    expect(
      mergeSharedColumns(
        [
          { id: 'artist', width: 120 },
          { id: 'title', width: 260 },
          { id: 'status', width: 116 },
        ],
        previous,
        new Set(['artist', 'title']),
      ),
    ).toEqual([
      { id: 'artist', width: 120 },
      { id: 'status', width: 175 },
      { id: 'title', width: 260 },
    ]);
    expect(previous[1].width).toBe(175);
  });
});
