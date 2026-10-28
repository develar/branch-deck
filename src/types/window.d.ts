import type { CachedModel, SubWindowOpen } from "@/lib/bindings"

declare global {
  interface Window {
    /** Sub-window payload injected by the `open_sub_window` command; replaced on re-open (see `init-data-updated`) */
    __INIT_DATA__?: unknown
    /** The open whose data `__INIT_DATA__` holds, injected and replaced with it; the page names it when its content settled */
    __SUB_WINDOW_OPEN__?: SubWindowOpen
    /** The settings by key, as the backend's writer had them when it built the window (`src-tauri/src/settings.rs`) */
    __TAURI_STORE__?: Record<string, unknown>
    /** The version of the writer `__TAURI_STORE__` was taken at: the page applies only the changes after it */
    __TAURI_STORE_VERSION__?: number
    /** How every window draws its title bar, injected by the window builder (`titleBar` in src/lib/title-bar.ts) */
    __BD_TITLE_BAR__?: import("@/lib/title-bar").TitleBar
    /**
     * The seed of the session of the selected project, the view of the frame an earlier run saved, injected into the main
     * window by the start of the app (`src-tauri/src/warm_start.rs`); none without a frame (`adoptCachedModel`)
     */
    __BD_MODEL__?: CachedModel
  }

  interface WindowEventMap {
    "init-data-updated": CustomEvent
  }
}

export {}
