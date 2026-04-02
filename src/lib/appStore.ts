import { load } from "@tauri-apps/plugin-store";

let store: Awaited<ReturnType<typeof load>> | null = null;

async function getStore() {
  if (!store) {
    store = await load("settings.json", { autoSave: true, defaults: {} });
  }
  return store;
}

export async function getSetting<T>(key: string, fallback: T): Promise<T> {
  try {
    const s = await getStore();
    const val = await s.get<T>(key);
    return val ?? fallback;
  } catch (e) {
    console.warn("[appStore] getSetting failed for key:", key, e);
    return fallback;
  }
}

export async function setSetting<T>(key: string, value: T): Promise<void> {
  try {
    const s = await getStore();
    await s.set(key, value);
  } catch (e) {
    console.error("[appStore] setSetting failed for key:", key, e);
  }
}
