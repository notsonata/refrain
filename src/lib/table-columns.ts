export interface TableColumn {
  id: string;
  label: string;
  width: number;
  minWidth: number;
  sortable?: boolean;
  draggable?: boolean;
  align?: 'left' | 'center' | 'right';
}

export function tableGridWidth(
  columns: TableColumn[],
  columnGap = 12,
  horizontalPadding = 16,
): number {
  return (
    columns.reduce((total, column) => total + column.width, 0) +
    Math.max(0, columns.length - 1) * columnGap +
    horizontalPadding
  );
}
