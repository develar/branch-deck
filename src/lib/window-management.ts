import { commands } from "@/lib/bindings"
import { commandFailure } from "@/lib/errors"
import { getStoreSnapshot } from "@/stores/settings/app-store"

interface SubWindowOptions<T = unknown> {
  windowId: string
  url: string
  title: string
  width?: number
  height?: number
  data: T
}

/**
 * Opens (or focuses and refreshes) a sub-window. The backend injects `data` as `window.__INIT_DATA__`
 * and the current settings as `window.__TAURI_STORE__`.
 */
export async function openSubWindow<T = unknown>(options: SubWindowOptions<T>): Promise<void> {
  const { windowId, url, title, width = 1400, height = 900, data } = options

  const result = await commands.openSubWindow({
    windowId,
    url,
    title,
    width,
    height,
    data: JSON.stringify(data),
    storeCache: JSON.stringify(getStoreSnapshot()),
    openedAt: Date.now(),
  })

  if (result.status === "error") {
    throw commandFailure(result.error)
  }
}
