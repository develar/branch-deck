declare global {
  interface Window {
    /** Sub-window payload injected by the `open_sub_window` command; replaced on re-open (see `init-data-updated`) */
    __INIT_DATA__?: unknown
    /** Settings snapshot injected by the backend before the page loads */
    __TAURI_STORE__?: Record<string, unknown>
    /** E2E overrides */
    __BRANCH_DECK_TEST_CONFIG__?: {
      copyTimeout?: number
    }
  }

  interface WindowEventMap {
    "init-data-updated": CustomEvent
  }
}

export {}
