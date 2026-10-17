import { lazy, Suspense, useLayoutEffect, useState } from "react"
import { Outlet } from "react-router"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useMainMenuKeys } from "@/hooks/useMainMenuKeys"
import { useZoom } from "@/hooks/useZoom"
import { applyPrimaryColor } from "@/lib/theme"
import { setColorSelectorDialogOpen, useIsColorSelectorDialogOpen } from "@/stores/color-selector-dialog"
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
  const colorSelectorOpen = useIsColorSelectorDialogOpen()
  useAppCommand("open_color_selector", () => {
    setColorSelectorDialogOpen(true)
  })
  // "Settings…" menu item (⌘,), also the menu of the Your Turn card
  const settingsOpen = useIsSettingsDialogOpen()
  useAppCommand("open_settings", () => {
    setSettingsDialogOpen(true)
  })
  // the keys of the main menu where it replaces the native menu (a custom title bar on Windows and Linux)
  useMainMenuKeys()
  const [isColorSelectorLoaded, setColorSelectorLoaded] = useState(false)
  if (colorSelectorOpen && !isColorSelectorLoaded) {
    setColorSelectorLoaded(true)
  }
  const [isSettingsLoaded, setSettingsLoaded] = useState(false)
  if (settingsOpen && !isSettingsLoaded) {
    setSettingsLoaded(true)
  }
  // "Create Pull Request…" of a branch in the main window (the review window has its own, `WindowCreatePullRequestDialog`)
  const createPullRequestOpen = useCreatePullRequestBranch() !== null
  const [isCreatePullRequestLoaded, setCreatePullRequestLoaded] = useState(false)
  if (createPullRequestOpen && !isCreatePullRequestLoaded) {
    setCreatePullRequestLoaded(true)
  }

  return (
    <TooltipProvider>
      <Outlet />
      <Suspense fallback={null}>
        {isColorSelectorLoaded && <ColorSelectorDialog open={colorSelectorOpen} onOpenChange={setColorSelectorDialogOpen} />}
        {isSettingsLoaded && <SettingsDialog open={settingsOpen} onOpenChange={setSettingsDialogOpen} />}
        {isCreatePullRequestLoaded && <CreatePullRequestDialog />}
      </Suspense>
      <Toaster />
    </TooltipProvider>
  )
}
