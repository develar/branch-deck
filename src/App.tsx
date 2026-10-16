import { lazy, Suspense, useLayoutEffect, useState } from "react"
import { Outlet } from "react-router"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useZoom } from "@/hooks/useZoom"
import { applyPrimaryColor } from "@/lib/theme"
import { useCreatePullRequestBranch } from "@/stores/create-pull-request-dialog"
import { setSettingsDialogOpen, useIsSettingsDialogOpen } from "@/stores/settings-dialog"
import { useAppSettings } from "@/stores/settings/appSettings"

// loaded on the first open, then kept mounted (a dialog animates and returns the focus when it closes); a sub-window
// never loads them
const ColorSelectorDialog = lazy(() => import("@/components/settings/ColorSelectorDialog").then(it => ({ default: it.ColorSelectorDialog })))
const SettingsDialog = lazy(() => import("@/components/settings/SettingsDialog").then(it => ({ default: it.SettingsDialog })))
const CreatePullRequestDialog = lazy(() => import("@/components/branch/CreatePullRequestDialog").then(it => ({ default: it.CreatePullRequestDialog })))

/** Root layout shared by the main window and sub-windows. */
export function App() {
  const primaryColor = useAppSettings(state => state.primaryColor)
  // layout effect: apply before paint to avoid a flash of the default primary colour
  useLayoutEffect(() => {
    applyPrimaryColor(primaryColor)
  }, [primaryColor])
  useZoom()

  // "Theme…" menu item
  const [colorSelectorOpen, setColorSelectorOpen] = useState(false)
  useTauriListen("open_color_selector", () => {
    setColorSelectorOpen(true)
  })
  // "Settings…" menu item (⌘,), also the menu of the Your Turn card
  const settingsOpen = useIsSettingsDialogOpen()
  useTauriListen("open_settings", () => {
    setSettingsDialogOpen(true)
  })
  const [isColorSelectorLoaded, setColorSelectorLoaded] = useState(false)
  if (colorSelectorOpen && !isColorSelectorLoaded) {
    setColorSelectorLoaded(true)
  }
  const [isSettingsLoaded, setSettingsLoaded] = useState(false)
  if (settingsOpen && !isSettingsLoaded) {
    setSettingsLoaded(true)
  }
  // "Create Pull Request…" of a branch (the main window; the review window asks it)
  const createPullRequestOpen = useCreatePullRequestBranch() !== null
  const [isCreatePullRequestLoaded, setCreatePullRequestLoaded] = useState(false)
  if (createPullRequestOpen && !isCreatePullRequestLoaded) {
    setCreatePullRequestLoaded(true)
  }

  return (
    <TooltipProvider>
      <Outlet />
      <Suspense fallback={null}>
        {isColorSelectorLoaded && <ColorSelectorDialog open={colorSelectorOpen} onOpenChange={setColorSelectorOpen} />}
        {isSettingsLoaded && <SettingsDialog open={settingsOpen} onOpenChange={setSettingsDialogOpen} />}
        {isCreatePullRequestLoaded && <CreatePullRequestDialog />}
      </Suspense>
      <Toaster />
    </TooltipProvider>
  )
}
