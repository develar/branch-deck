/**
 * What the page of a view needs before its main (`bd_bootstrap {view}` of `brd mcp`), where the desktop app has the init
 * script of its window: the settings as the writer of `brd mcp` has them (`__TAURI_STORE__` at
 * `__TAURI_STORE_VERSION__`), the warm frame of the main view (`__BD_MODEL__`), the data of an open of a sub-window
 * (`__INIT_DATA__` and `__SUB_WINDOW_OPEN__`) and the title bar (`__BD_TITLE_BAR__`).
 */
import type { CachedModel, SubWindowOpen } from "@/lib/bindings"
import type { TitleBar } from "@/lib/title-bar"

/** The answer of `bd_bootstrap` (`structuredContent`) */
export interface Bootstrap {
  readonly settings?: { readonly values?: Readonly<Record<string, unknown>>; readonly version?: number } | null
  readonly warmFrame?: CachedModel | null
  /** The data of the open as `open_sub_window` got it, the JSON text or its value */
  readonly initData?: { readonly data?: unknown; readonly open?: SubWindowOpen | null } | null
  readonly titleBar?: TitleBar | null
}

/** The path of the page of a view in the router of the page (`src/router.ts`): `main` is `/` */
export function pagePathOf(view: string): string {
  return view === "main" ? "/" : `/${view}`
}

/** Sets the globals of the init script of a window from the answer of `bd_bootstrap` */
export function applyBootstrap(target: Window, bootstrap: Bootstrap): void {
  const settings = bootstrap.settings
  if (settings?.values !== undefined) {
    target.__TAURI_STORE__ = { ...settings.values }
    target.__TAURI_STORE_VERSION__ = settings.version ?? 0
  }
  if (bootstrap.warmFrame != null) {
    target.__BD_MODEL__ = bootstrap.warmFrame
  }
  const init = bootstrap.initData
  if (init != null) {
    if (init.data !== undefined && init.data !== null) {
      target.__INIT_DATA__ = typeof init.data === "string" ? (JSON.parse(init.data) as unknown) : init.data
    }
    if (init.open != null) {
      target.__SUB_WINDOW_OPEN__ = init.open
    }
  }
  // the IDE draws the frame of every place: the row of the page is no title bar, and the app updates with the desktop app
  target.__BD_TITLE_BAR__ = bootstrap.titleBar ?? { kind: "native", checkForUpdates: false }
}
