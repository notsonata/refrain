export type OverflowMenuIcon =
  'check' | 'close' | 'copy' | 'download' | 'folder' | 'link' | 'refresh';

export interface OverflowMenuItem {
  label: string;
  icon: OverflowMenuIcon;
  action: () => void | Promise<void>;
  disabled?: boolean;
}
