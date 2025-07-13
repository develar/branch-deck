import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { emit } from '@tauri-apps/api/event'

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
    data
  } = options

  // Create the sub-window
  const subWindow = new WebviewWindow(windowId, {
    url,
    title,
    width,
    height,
    center: true,
    resizable: true,
    skipTaskbar: true,
  })
  
  // Listen for the ready signal from the window
  subWindow.once(`${windowId}-ready`, async () => {
    // Send the data when the window signals it's ready
    await emit(`init-${windowId}-data`, data)
  })
}