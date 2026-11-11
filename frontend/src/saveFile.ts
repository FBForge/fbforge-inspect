import { invoke, isTauri } from '@tauri-apps/api/core';

/** All native writes require the existing explicit save dialog. */
export async function saveFile(content: string, format = 'json') {
  if (isTauri()) return invoke<boolean>('save_report', { content, format });
  const url = URL.createObjectURL(
    new Blob([content], {
      type: format === 'json' ? 'application/json' : 'text/plain;charset=utf-8',
    }),
  );
  const link = document.createElement('a');
  link.href = url;
  link.download = `fbforge-inspect.${format}`;
  link.click();
  URL.revokeObjectURL(url);
  return true;
}
