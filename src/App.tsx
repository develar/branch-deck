import { Outlet } from "react-router"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useMainMenuKeys } from "@/hooks/useMainMenuKeys"
import { useZoom } from "@/hooks/useZoom"

/** The root of every window: the page of its route, the tooltips, the toasts, the zoom and the keys of the main menu. */
export function App() {
  useZoom()
  // the keys of the main menu where it replaces the native menu (a custom title bar on Windows and Linux)
  useMainMenuKeys()

  return (
    <TooltipProvider>
      <Outlet />
      <Toaster />
    </TooltipProvider>
  )
}
