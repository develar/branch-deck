import type { SubWindowOpen } from "@/lib/bindings"

declare global {
  interface Window {
    /** Sub-window payload injected by the `open_sub_window` command; replaced on re-open (see `init-data-updated`) */
    __INIT_DATA__?: unknown
    /** The open whose data `__INIT_DATA__` holds, injected and replaced with it; the page names it when its content settled */
    __SUB_WINDOW_OPEN__?: SubWindowOpen
    /** Settings snapshot injected by the backend before the page loads */
    __TAURI_STORE__?: Record<string, unknown>
    /** How every window draws its title bar, injected by the window builder (`titleBar` in src/lib/title-bar.ts) */
    __BD_TITLE_BAR__?: import("@/lib/title-bar").TitleBar
  }

  interface WindowEventMap {
    "init-data-updated": CustomEvent
  }
}

export {}
