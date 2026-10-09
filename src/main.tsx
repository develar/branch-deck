import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { RouterProvider } from "react-router"
import "./index.css"
import "@/lib/sub-window"
import { router } from "@/router"
import { initializeStoreHandlers } from "@/stores/settings/app-store"

// main window: serve settings writes from sub-windows (no-op in sub-windows); module level, not an effect
initializeStoreHandlers()

const rootElement = document.getElementById("root")
if (rootElement == null) {
  throw new Error("Root element #root not found")
}

createRoot(rootElement).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
