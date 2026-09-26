export interface TableColumn {
  id: string;
  label: string;
  width: number;
  minWidth: number;
  sortable?: boolean;
  draggable?: boolean;
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
  { id: 'title', label: 'Title', width: 300, minWidth: 220, sortable: true },
  { id: 'artist', label: 'Artist', width: 150, minWidth: 100, sortable: true },
  { id: 'album', label: 'Album', width: 170, minWidth: 110, sortable: true },
  { id: 'year', label: 'Year', width: 58, minWidth: 48, sortable: true },
  {
    id: 'duration',
    label: 'Duration',
    width: 72,
    minWidth: 60,
    sortable: true,
  },
  { id: 'format', label: 'Format', width: 62, minWidth: 54, sortable: true },
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
  width: 54,
  minWidth: 54,
  draggable: false,
  align: 'center',
};

const spotifyOrderColumn: TableColumn = {
  ...localOrderColumn,
  label: 'Order',
  sortable: true,
};

function cloneSharedTrackColumns(): TableColumn[] {
  return sharedTrackColumns.map((column) => ({ ...column }));
}

export function createLocalTrackColumns(): TableColumn[] {
  return [
    { ...localOrderColumn },
    ...cloneSharedTrackColumns(),
    {
      id: 'playlists',
      label: 'Playlists',
      width: 170,
      minWidth: 120,
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
      column.draggable === false
        ? `${column.width}px`
        : `minmax(0, ${Math.max(1, column.width)}fr)`,
    )
    .join(' ');
}
