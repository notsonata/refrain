export type OverflowMenuIcon =
  | 'check'
  | 'close'
  | 'copy'
  | 'download'
  | 'folder'
  | 'link'
  | 'refresh'
  | 'sort-asc'
  | 'sort-desc'
  | 'playlist';

export interface OverflowMenuItem {
  label: string;
  icon: OverflowMenuIcon;
  action: () => void | Promise<void>;
  disabled?: boolean;
  separatorBefore?: boolean;
}
