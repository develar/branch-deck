declare global {
  interface Window {
    __INIT_DATA__?: unknown
    __TAURI_STORE__?: Record<string, unknown>
  }
}

export {}
