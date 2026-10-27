export interface TableColumn {
  id: string;
  label: string;
  width: number;
  minWidth: number;
  sortable?: boolean;
  draggable?: boolean;
  resizable?: boolean;
  fluid?: boolean;
  fixedWidth?: boolean;
  gridMinWidth?: number;
  legacyDefaultWidth?: number;
  align?: 'left' | 'center' | 'right';
}

export const sharedTrackColumnIds = [
  'title',
  'artist',
  'album',
  'year',
  'duration',
  'format',
  'status',
] as const;

const sharedTrackColumns: TableColumn[] = [
  { id: 'title', label: 'Title', width: 286, minWidth: 210, sortable: true },
  { id: 'artist', label: 'Artist', width: 146, minWidth: 100, sortable: true },
  { id: 'album', label: 'Album', width: 156, minWidth: 106, sortable: true },
  { id: 'year', label: 'Year', width: 68, minWidth: 64, sortable: true },
  {
    id: 'duration',
    label: 'Duration',
    width: 84,
    minWidth: 78,
    sortable: true,
  },
  { id: 'format', label: 'Format', width: 72, minWidth: 68, sortable: true },
  { id: 'status', label: 'Status', width: 126, minWidth: 108, sortable: true },
];

const actionsColumn: TableColumn = {
  id: 'actions',
  label: '',
  width: 30,
  minWidth: 30,
  draggable: false,
  align: 'center',
};

const localOrderColumn: TableColumn = {
  id: 'order',
  label: '',
  width: 22,
  minWidth: 22,
  draggable: false,
  resizable: false,
  align: 'center',
};

const spotifyOrderColumn: TableColumn = {
  ...localOrderColumn,
  label: 'Order',
  width: 54,
  minWidth: 54,
  resizable: true,
  sortable: true,
};

function cloneSharedTrackColumns(): TableColumn[] {
  return sharedTrackColumns.map((column) => ({ ...column }));
}

export function createLocalTrackColumns(): TableColumn[] {
  return [
    { ...localOrderColumn },
    ...cloneSharedTrackColumns().map((column) =>
      column.id === 'status'
        ? {
            ...column,
            width: 92,
            minWidth: 64,
            fixedWidth: true,
          }
        : column,
    ),
    {
      id: 'playlists',
      label: 'Playlists',
      width: 170,
      minWidth: 48,
      gridMinWidth: 48,
      fluid: true,
      sortable: true,
      draggable: false,
    },
    { ...actionsColumn },
  ];
}

export function createSpotifyTrackColumns(): TableColumn[] {
  return [
    { ...spotifyOrderColumn },
    ...cloneSharedTrackColumns(),
    {
      id: 'tracking',
      label: 'Tracking',
      width: 112,
      minWidth: 96,
      sortable: true,
      draggable: false,
    },
    { ...actionsColumn },
  ];
}

export function tableGridTemplate(columns: TableColumn[]): string {
  return columns
    .map((column) =>
      (column.draggable === false && !column.fluid) || column.fixedWidth
        ? `${column.width}px`
        : `minmax(${column.gridMinWidth ?? 0}px, ${Math.max(1, column.width)}fr)`,
    )
    .join(' ');
}

export function restoredColumnWidth(
  column: TableColumn,
  savedWidth: number | undefined,
): number {
  if (column.resizable === false) return column.width;
  const width =
    typeof savedWidth === 'number' &&
    Number.isFinite(savedWidth) &&
    savedWidth !== column.legacyDefaultWidth
      ? savedWidth
      : column.width;
  return Math.max(column.minWidth, width);
}

export function migrateLegacyColumns(
  savedColumns: Array<{ id: string; width?: number }> | undefined,
  resetWidthIds: readonly string[],
): Array<{ id: string; width?: number }> | undefined {
  if (!savedColumns) return undefined;
  const reset = new Set(resetWidthIds);
  return savedColumns.map((saved) =>
    reset.has(saved.id) ? { ...saved, width: undefined } : saved,
  );
}

export function mergeSharedColumns(
  columns: Array<{ id: string; width: number }>,
  previous: Array<{ id: string; width?: number }>,
  sharedIds: ReadonlySet<string>,
): Array<{ id: string; width?: number }> {
  const pending = columns.filter((column) => sharedIds.has(column.id));
  const merged: Array<{ id: string; width?: number }> = [];

  for (const column of previous) {
    if (!sharedIds.has(column.id)) {
      merged.push(column);
    } else {
      const next = pending.shift();
      if (next) merged.push(next);
    }
  }

  return [...merged, ...pending];
}
