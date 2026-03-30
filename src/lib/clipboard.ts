import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/** Copy text to clipboard using native Tauri plugin (works even when window is not focused). */
export async function copyToClipboard(text: string): Promise<void> {
  try {
    await writeText(text);
  } catch {
    // Fallback to browser API if plugin fails
    await navigator.clipboard.writeText(text);
  }
}
