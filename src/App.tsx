import { useLayoutEffect, useState } from "react"
import { Outlet } from "react-router"
import { ColorSelectorDialog } from "@/components/settings/ColorSelectorDialog"
import { SettingsDialog } from "@/components/settings/SettingsDialog"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useZoom } from "@/hooks/useZoom"
import { applyPrimaryColor } from "@/lib/theme"
import { setSettingsDialogOpen, useIsSettingsDialogOpen } from "@/stores/settings-dialog"
import { useAppSettings } from "@/stores/settings/appSettings"

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

  return (
    <TooltipProvider>
      <Outlet />
      <ColorSelectorDialog open={colorSelectorOpen} onOpenChange={setColorSelectorOpen} />
      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsDialogOpen} />
      <Toaster />
    </TooltipProvider>
  )
}
