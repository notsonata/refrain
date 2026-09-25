import { open } from '@tauri-apps/plugin-dialog';

export async function chooseLibraryRoot(
  currentPath?: string,
): Promise<string | null> {
  return open({
    directory: true,
    multiple: false,
    title: 'Choose library folder',
    defaultPath: currentPath || undefined,
  });
}
