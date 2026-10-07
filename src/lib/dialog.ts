import { open, save } from '@tauri-apps/plugin-dialog';

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

export async function choosePlaylistFile(
  playlistName: string,
  currentPath?: string,
): Promise<string | null> {
  const safeName =
    playlistName
      .trim()
      .replace(/[\\/:*?"<>|]+/g, '-')
      .replace(/\s+/g, ' ') || 'playlist';
  return save({
    title: 'Choose playlist file',
    defaultPath: currentPath || `${safeName}.m3u8`,
    filters: [{ name: 'M3U playlist', extensions: ['m3u8', 'm3u'] }],
  });
}
