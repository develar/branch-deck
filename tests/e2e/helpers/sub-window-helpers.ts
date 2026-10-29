import type { Page } from "@playwright/test"
import { PAGE_COMMANDS, tauriMockScript } from "../setup/tauri-mocks"
import { TEST_SERVER_URL } from "./test-server-url"

/** A sub-window the page opened (`open_sub_window`, recorded by the desktop of the test server, `TestDesktop`) */
export interface OpenedSubWindow<T = unknown> {
  windowId: string
  url: string
  title: string
  data: T
  /** The settings the window starts from: the writer's copy when it was opened, as the backend builds it */
  store?: Record<string, unknown>
  /** The version of the writer `store` was taken at (`__TAURI_STORE_VERSION__`) */
  storeVersion?: number
}

/**
 * What the desktop of the test server recorded for the repository of `page`: the opens (with the window and the page of
 * it that asked: a window's pages are counted by their loads), the window menus, and the page each window shows now
 */
interface RecordedDesktop {
  opens: (OpenedSubWindow & { opener: string; page: number })[]
  menus: string[]
  pages: Record<string, number>
}

/** The label of the window of `page`: `main`, or that of a linked sub-window (`__e2eWindowLabel`) */
async function windowLabelOf(page: Page): Promise<string> {
  return page.evaluate(() => (window as unknown as { __e2eWindowLabel?: string }).__e2eWindowLabel ?? "main")
}

/** The test repository of `page` (`?repoId=`) */
function repositoryOf(page: Page): string {
  const repoId = new URL(page.url()).searchParams.get("repoId")
  if (repoId === null) {
    throw new Error(`The page ${page.url()} names no test repository`)
  }
  return repoId
}

async function recordedDesktop(page: Page): Promise<RecordedDesktop> {
  const response = await fetch(`${TEST_SERVER_URL}/repositories/${repositoryOf(page)}/windows`)
  if (!response.ok) {
    throw new Error(`Cannot read the windows of the test repository: ${response.status}`)
  }
  return (await response.json()) as RecordedDesktop
}

/**
 * The sub-windows the page opened, in order (`open_sub_window`, as the backend got each): those its window asked for
 * since the page loaded, so a page shown again in the same tab (`openSubWindowPage`, a reload) starts with none
 */
export async function getOpenedSubWindows<T = unknown>(page: Page): Promise<OpenedSubWindow<T>[]> {
  const label = await windowLabelOf(page)
  const { opens, pages } = await recordedDesktop(page)
  return opens.filter(open => open.opener === label && open.page === pages[label]).map(({ opener: _opener, page: _page, ...open }) => open as OpenedSubWindow<T>)
}

/** How often the window of `page` asked for the window manager's menu (`show_window_menu`) */
export async function getWindowMenus(page: Page): Promise<number> {
  const label = await windowLabelOf(page)
  return (await recordedDesktop(page)).menus.filter(window => window === label).length
}

/**
 * The window of a page a test opens is built, as the app builds it before its page loads: a sub-window from the
 * snapshot of the settings at `storeVersion` (its load takes the changes after it, `sub_window_loaded`), the main window
 * (it catches up itself); `openId` is the open the page names (`__SUB_WINDOW_OPEN__`)
 */
export async function registerWindowBuild(repoId: string, window: { label: string; page: "main" | "sub"; storeVersion?: number; openId: number }): Promise<void> {
  const response = await fetch(`${TEST_SERVER_URL}/repositories/${repoId}/windows/built`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(window),
  })
  if (!response.ok) {
    throw new Error(`Cannot build the window ${window.label} of the test repository: ${response.status}`)
  }
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

/**
 * The payloads of the events named `event` that the page emitted for the windows (`emit`, `emit_to`) or heard from the
 * backend (`settings_changed`, `activity-changed`, …, on the test server's event stream of the page), in order, as the
 * Tauri mock recorded them
 */
export async function getEmittedEvents<T = unknown>(page: Page, event: string): Promise<T[]> {
  return page.evaluate(
    name => ((window as unknown as { __e2eEmittedEvents?: { event: string; payload: T }[] }).__e2eEmittedEvents ?? []).filter(it => it.event === name).map(it => it.payload),
    event,
  )
}

/**
 * Opens a sub-window as a page of its own beside the main window's page, under its own label (`__e2eWindowLabel`), as
 * the app opens it: its route with the recorded data as `__INIT_DATA__`, the recorded settings of the open (else the
 * main window's copy now) as `__TAURI_STORE__` with their version, its webview zoomed to their level as the window
 * builder zooms it (`__e2eWebviewZoom`, `apply_zoom`); once it listens, the page's load takes the changes made since
 * (`sub_window_loaded`, from the version of its snapshot). The events of a page reach the other through the test
 * server, as the events of the backend reach both (the event stream of each page), so the link of the window works end
 * to end: the window asks, the main window runs it against the test server and answers. Escape that closes the window is
 * the `close` of `__e2eWindowCalls` of the window's page. `beforeLoad` routes what the window itself asks the backend.
 * The window's page is open 1 (`__SUB_WINDOW_OPEN__`), its window built for it on the test server before it loads
 * (`registerWindowBuild`); `getSubWindowReadyOpen` tells when its content settled (`observeWindowStages`).
 */
export async function openLinkedSubWindow(main: Page, subWindow: OpenedSubWindow, { beforeLoad }: { beforeLoad?: (page: Page) => Promise<unknown> } = {}): Promise<Page> {
  const repoId = repositoryOf(main)
  const current = await main.evaluate(() => {
    const target = window as unknown as { __TAURI_STORE__?: unknown; __TAURI_STORE_VERSION__?: number }
    return { store: target.__TAURI_STORE__ ?? {}, version: target.__TAURI_STORE_VERSION__ ?? 0 }
  })
  const store = subWindow.store ?? current.store
  const version = subWindow.store === undefined ? current.version : (subWindow.storeVersion ?? 0)
  const page = await main.context().newPage()
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
  await page.addInitScript(tauriMockScript, PAGE_COMMANDS)
  await observeWindowStages(page)
  await registerWindowBuild(repoId, { label: subWindow.windowId, page: "sub", storeVersion: version, openId: 1 })
  // the routes of the window's own commands (its fetch), before it asks
  await beforeLoad?.(page)
  await page.goto(`${subWindow.url}?repoId=${repoId}`)
  return page
}

/** A call of a command the page made (its arguments, as the page sent them) and what `measure` found in it then */
export interface ObservedCall<T> {
  args: Record<string, unknown>
  measured: T
}

/**
 * Records each call of `command` of the page with what `measure` finds in its DOM when the call reaches the backend,
 * and lets it go on to the test server: what the page had when it said a stage of its window
 */
export async function observeCommand<T>(page: Page, command: string, measure: () => T): Promise<ObservedCall<T>[]> {
  const calls: ObservedCall<T>[] = []
  await page.route(`**/invoke/${command}`, async route => {
    try {
      const args = (route.request().postDataJSON() ?? {}) as Record<string, unknown>
      calls.push({ args, measured: await page.evaluate(measure) })
    } catch (error) {
      // the test ended as the call went out (its window says a stage as it closes): nothing is left to measure, and
      // the call goes nowhere
      if (!page.isClosed()) {
        throw error
      }
    } finally {
      if (!page.isClosed()) {
        await route.fallback()
      }
    }
  })
  return calls
}

/** What the page had in its DOM when it said its shell is drawn (`sub_window_drawn`) */
export interface WindowShell {
  titleRows: number
  loadingTables: number
  branchRows: number
}

/** What a page said of the stages of its window (`observeWindowStages`) */
interface WindowStages {
  drawn: ObservedCall<WindowShell>[]
  ready: ObservedCall<number>[]
}

const windowStages = new WeakMap<Page, WindowStages>()

/**
 * Records the stages the page says of its window, which the backend shows it by: its shell (`sub_window_drawn`, with
 * the title rows, the loading tables and the branch rows it had) and its settled content (`sub_window_ready`, with the
 * branch rows it had); before the page loads. `openLinkedSubWindow` observes every window it opens
 */
export async function observeWindowStages(page: Page): Promise<void> {
  const drawn = await observeCommand(page, "sub_window_drawn", () => ({
    titleRows: document.querySelectorAll("header[data-title-bar]").length,
    loadingTables: document.querySelectorAll("[data-testid=branch-table-loading]").length,
    branchRows: document.querySelectorAll("[data-testid=branch-row]").length,
  }))
  const ready = await observeCommand(page, "sub_window_ready", () => document.querySelectorAll("[data-testid=branch-row]").length)
  windowStages.set(page, { drawn, ready })
}

function stagesOf(page: Page): WindowStages {
  const stages = windowStages.get(page)
  if (stages === undefined) {
    throw new Error("The stages of the window of the page are not observed: call observeWindowStages before it loads")
  }
  return stages
}

/** The open whose content the page said settled last (`sub_window_ready`), or null before it said it */
export async function getSubWindowReadyOpen(page: Page): Promise<number | null> {
  return (stagesOf(page).ready.at(-1)?.args.openId as number | undefined) ?? null
}

/** The branch rows the page had when it said its content settled last (`sub_window_ready`), or null before it said it */
export async function getWindowReadyRows(page: Page): Promise<number | null> {
  return stagesOf(page).ready.at(-1)?.measured ?? null
}

/** What the page had when it said its shell is drawn (`sub_window_drawn`), or null before it said it */
export async function getWindowShell(page: Page): Promise<WindowShell | null> {
  return stagesOf(page).drawn.at(-1)?.measured ?? null
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
