import { commands } from "~/utils/bindings"

interface SubWindowOptions<T = unknown> {
  windowId: string
  url: string
  title: string
  width?: number
  height?: number
  data: T
}

export async function openSubWindow<T = unknown>(options: SubWindowOptions<T>) {
  const {
    windowId,
    url,
    title,
    width = 1400,
    height = 900,
    data,
  } = options

  // Get the current store snapshot
  const storeCache = appStore.getStoreSnapshot()

  // Use the Rust command to create/focus the window
  const result = await commands.openSubWindow({
    windowId,
    url,
    title,
    width,
    height,
    data: JSON.stringify(data),
    storeCache: JSON.stringify(storeCache),
  })

  if (result.status === "error") {
    throw new Error(result.error.message)
  }
}
