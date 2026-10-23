import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { RouterProvider } from "react-router"
import "./index.css"
import "@/lib/sub-window"
import { startReviewWindowReads } from "@/lib/review-window-reads"
import { router } from "@/router"

// a review window reads its subject while its route chunk loads
startReviewWindowReads()

const rootElement = document.getElementById("root")
if (rootElement == null) {
  throw new Error("Root element #root not found")
}

createRoot(rootElement).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
