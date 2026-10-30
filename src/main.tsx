import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { RouterProvider } from "react-router"
import "./index.css"
import "@/lib/sub-window"
// before the router: a review window asks for its reads before the chunk of its page
import "@/lib/review-window-start"
import { router } from "@/router"

const rootElement = document.getElementById("root")
if (rootElement == null) {
  throw new Error("Root element #root not found")
}

createRoot(rootElement).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
