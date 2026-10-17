import { lazy, Suspense, useState } from "react"
import { Outlet } from "react-router"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useMainMenuKeys } from "@/hooks/useMainMenuKeys"
import { useZoom } from "@/hooks/useZoom"
import { setSettingsDialogOpen, useIsSettingsDialogOpen } from "@/stores/settings-dialog"

// loaded on the first open, then kept mounted (a dialog animates and returns the focus when it closes)
const SettingsDialog = lazy(() => import("@/components/settings/SettingsDialog").then(it => ({ default: it.SettingsDialog })))

/** The root of every window: the page of its route, the tooltips, the toasts, the zoom and the main menu. */
export function App() {
  useZoom()

  // "Settings…" menu item (⌘,), also the menu of the Your Turn card
  const settingsOpen = useIsSettingsDialogOpen()
  useAppCommand("open_settings", () => {
    setSettingsDialogOpen(true)
  })
  // the keys of the main menu where it replaces the native menu (a custom title bar on Windows and Linux)
  useMainMenuKeys()
  const [isSettingsLoaded, setSettingsLoaded] = useState(false)
  if (settingsOpen && !isSettingsLoaded) {
    setSettingsLoaded(true)
  }

  return (
    <TooltipProvider>
      <Outlet />
      <Suspense fallback={null}>{isSettingsLoaded && <SettingsDialog open={settingsOpen} onOpenChange={setSettingsDialogOpen} />}</Suspense>
      <Toaster />
    </TooltipProvider>
  )
}
