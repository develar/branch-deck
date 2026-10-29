import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"
import { showTheme, shownTheme } from "@/lib/theme"
import type { Theme } from "@/lib/theme"
import { appSettingsStore, useAppSettings } from "@/stores/settings/appSettings"

const SYSTEM_DARK = "(prefers-color-scheme: dark)"

/** The theme of the window's frame: it was built following the system, so it changes only once the setting differs */
let windowTheme: Theme = "system"

function show(theme: Theme): void {
  showTheme(document.documentElement, shownTheme(theme, matchMedia(SYSTEM_DARK).matches))
}

// before the first render, from the snapshot the page starts from: no frame of the other theme
if (typeof document !== "undefined") {
  show(appSettingsStore.getState().theme)
}

/**
 * The theme of Settings → Appearance in this window: the tokens of the page (`data-theme`), following the system while
 * the setting is System, and the frame of the window (the theme of Tauri, which a sub-window's background reads too)
 */
export function useTheme(): void {
  const theme = useAppSettings(state => state.theme)
  useEffect(() => {
    show(theme)
    const media = matchMedia(SYSTEM_DARK)
    const follow = () => show(theme)
    media.addEventListener("change", follow)
    if (theme !== windowTheme) {
      windowTheme = theme
      getCurrentWindow()
        .setTheme(theme === "system" ? null : theme)
        .catch((error: unknown) => logError("Cannot set the theme of the window", error))
    }
    return () => media.removeEventListener("change", follow)
  }, [theme])
}
