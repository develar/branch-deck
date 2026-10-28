import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { SetupEntry } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { appSettingsStore } from "@/stores/settings/appSettings"

/*
 * The Setup dot (spec/repository/setup-attention.spec.md): what of the app and the CLIs waits for the user, as Rust
 * derives it (`get_setup_items`) from the downloaded update, the last statuses of the CLIs and the YouTrack sign-in,
 * without what the user skipped (`appSettings.setupSkips`).
 */

/** The main window reads the CLIs again this often: their releases and sign-ins change without the app */
const TOOLS_READ_INTERVAL_MS = 6 * 60 * 60 * 1000

const setupStore = createStore<{ readonly entries: readonly SetupEntry[] }>(() => ({ entries: [] }))

export function useSetupEntries(): readonly SetupEntry[] {
  return useStore(setupStore, state => state.entries)
}

let reading: Promise<void> | null = null
let readAgain = false

/** Asks Rust for the items again; a read while one runs is asked once more after it. A failure keeps the last items */
export function refreshSetup(): Promise<void> {
  if (reading !== null) {
    readAgain = true
    return reading
  }
  reading = commands
    .getSetupItems(appSettingsStore.getState().setupSkips)
    .then(result => {
      if (result.status === "ok") {
        setupStore.setState({ entries: result.data })
      } else {
        logError("Cannot read what the setup needs", result.error)
      }
    })
    .finally(() => {
      reading = null
      if (readAgain) {
        readAgain = false
        void refreshSetup()
      }
    })
  return reading
}

/**
 * Keeps the items of a window current: read on mount (the first read of the app runs the CLIs, later ones take their
 * last statuses), when the CLIs were read again (`setup-changed`), on a sign-in or a sign-out (`auth-changed`) and on a
 * skip. The main window also reads the CLIs every 6 hours (`readTools`)
 */
export function useSetupTracking({ readTools }: { readTools: boolean }): void {
  const skips = useStore(appSettingsStore, state => state.setupSkips)
  useEffect(() => {
    void refreshSetup()
  }, [skips])
  useTauriListen("setup-changed", () => void refreshSetup())
  useTauriListen("auth-changed", () => void refreshSetup())
  useEffect(() => {
    if (!readTools) {
      return
    }
    // the read answers `setup-changed`, which reads the items
    const read = () => void commands.cliToolsStatus()
    const every = setInterval(read, TOOLS_READ_INTERVAL_MS)
    return () => clearInterval(every)
  }, [readTools])
}
