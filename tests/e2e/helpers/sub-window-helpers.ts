import type { BrowserContext, Page } from "@playwright/test"
import { tauriMockScript } from "../setup/tauri-mocks"

/** A sub-window the page opened (`open_sub_window`, recorded by the Tauri mock) */
export interface OpenedSubWindow<T = unknown> {
  windowId: string
  url: string
  title: string
  data: T
  /** The settings the window starts from: the backend's copy when it was opened, recorded by the mock */
  store?: Record<string, unknown>
  /** The version of the writer `store` was taken at (`__TAURI_STORE_VERSION__`) */
  storeVersion?: number
}

export async function getOpenedSubWindows<T = unknown>(page: Page): Promise<OpenedSubWindow<T>[]> {
  return page.evaluate(() => (window as unknown as { __e2eSubWindows?: OpenedSubWindow<T>[] }).__e2eSubWindows ?? [])
}

/**
 * Shows a sub-window in the page, as the app opens it: its route with the data as `window.__INIT_DATA__`. The mock
 * keeps the label `main` (another label would wait for a main window), so what only a sub-window does is not covered.
 */
export async function openSubWindowPage(page: Page, subWindow: OpenedSubWindow): Promise<void> {
  const repoId = new URL(page.url()).searchParams.get("repoId")
  await page.addInitScript(data => {
    ;(window as unknown as { __INIT_DATA__: unknown }).__INIT_DATA__ = data
  }, subWindow.data)
  await page.goto(`${subWindow.url}?repoId=${repoId}`)
}

/** The payloads of the events the page emitted for other windows (`emit`, recorded by the Tauri mock), in order */
export async function getEmittedEvents<T = unknown>(page: Page, event: string): Promise<T[]> {
  return page.evaluate(
    name => ((window as unknown as { __e2eEmittedEvents?: { event: string; payload: T }[] }).__e2eEmittedEvents ?? []).filter(it => it.event === name).map(it => it.payload),
    event,
  )
}

/** The pages of each context that relay their events to each other (`__e2eRelay`, exposed once per context) */
const linkedPages = new WeakMap<BrowserContext, Page[]>()

/**
 * Opens a sub-window as a page of its own beside the main window's page, under its own label (`__e2eWindowLabel`), as
 * the app opens it: its route with the recorded data as `__INIT_DATA__`, the recorded settings of the open (else the
 * main window's copy now) as `__TAURI_STORE__` with their version, its webview zoomed to their level as the window
 * builder zooms it (`__e2eWebviewZoom`, `apply_zoom`); once it listens, the page's load takes the changes made since
 * (`sub_window_loaded`, from the version of its snapshot). The events one page emits reach the other (`__e2eRelay`), so
 * the link of the window works end to end: the window asks, the main window runs it against the test server and answers. Escape that closes the window is
 * the `close` of `__e2eWindowCalls` of the window's page. `beforeLoad` routes what the window itself asks the backend.
 * The window's page is open 1 (`__SUB_WINDOW_OPEN__`); `getSubWindowReadyOpen` tells when its content settled.
 */
export async function openLinkedSubWindow(main: Page, subWindow: OpenedSubWindow, { beforeLoad }: { beforeLoad?: (page: Page) => Promise<unknown> } = {}): Promise<Page> {
  const context = main.context()
  let pages = linkedPages.get(context)
  if (pages === undefined) {
    const linked: Page[] = [main]
    pages = linked
    linkedPages.set(context, linked)
    await context.exposeBinding("__e2eRelay", async ({ page: source }, event: string, payload: unknown) => {
      for (const page of linked) {
        if (page !== source && !page.isClosed()) {
          await page.evaluate(([name, value]) => (window as unknown as { __e2eEmitTauriEvent: (event: string, payload: unknown) => void }).__e2eEmitTauriEvent(name, value), [
            event,
            payload,
          ] as const)
        }
      }
    })
  }
  const repoId = new URL(main.url()).searchParams.get("repoId")
  const current = await main.evaluate(() => {
    const target = window as unknown as { __TAURI_STORE__?: unknown; __TAURI_STORE_VERSION__?: number }
    return { store: target.__TAURI_STORE__ ?? {}, version: target.__TAURI_STORE_VERSION__ ?? 0 }
  })
  const store = subWindow.store ?? current.store
  const version = subWindow.store === undefined ? current.version : (subWindow.storeVersion ?? 0)
  const page = await context.newPage()
  await page.addInitScript(
    ({ label, data, settings, version }) => {
      const target = window as unknown as {
        __e2eWindowLabel: string
        __INIT_DATA__: unknown
        __TAURI_STORE__: unknown
        __TAURI_STORE_VERSION__: number
        __SUB_WINDOW_OPEN__: unknown
        __e2eWebviewZoom?: number
      }
      target.__e2eWindowLabel = label
      target.__INIT_DATA__ = data
      target.__TAURI_STORE__ = settings
      target.__TAURI_STORE_VERSION__ = version
      // the builder zooms the webview to the level of the snapshot, before the page's scripts (`apply_zoom`)
      const zoom = (settings as { appSettings?: { zoom?: number } }).appSettings?.zoom
      if (zoom !== undefined && zoom !== 1) {
        target.__e2eWebviewZoom = zoom
      }
      // the open of the window, as the backend numbers it: the page names it when its content settled
      target.__SUB_WINDOW_OPEN__ = { id: 1, openedAt: Date.now() }
    },
    { label: subWindow.windowId, data: subWindow.data, settings: store, version },
  )
  await page.addInitScript(tauriMockScript)
  pages.push(page)
  // the routes of the window's own commands (its fetch), before it asks
  await beforeLoad?.(page)
  await page.goto(`${subWindow.url}?repoId=${repoId}`)
  return page
}

/** The open whose content the sub-window's page said settled (`sub_window_ready`, recorded by the Tauri mock), or null */
export async function getSubWindowReadyOpen(page: Page): Promise<number | null> {
  return page.evaluate(() => (window as unknown as { __e2eSubWindowReady?: number }).__e2eSubWindowReady ?? null)
}

/**
 * Opens the shown sub-window again with the data of a later open, as the backend does (`SubWindowUpdate::script`): the
 * new `__INIT_DATA__` and open, then `init-data-updated`; the settings the page has heard already (`settings_changed`)
 */
export async function reopenLinkedSubWindow(window: Page, subWindow: OpenedSubWindow): Promise<void> {
  await window.evaluate(
    ({ data }) => {
      const target = globalThis as unknown as { __INIT_DATA__: unknown; __SUB_WINDOW_OPEN__: { id: number; openedAt: number } }
      target.__INIT_DATA__ = data
      target.__SUB_WINDOW_OPEN__ = { id: target.__SUB_WINDOW_OPEN__.id + 1, openedAt: Date.now() }
      globalThis.dispatchEvent(new CustomEvent("init-data-updated"))
    },
    { data: subWindow.data },
  )
}
