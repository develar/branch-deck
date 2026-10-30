import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import "./index.css"
import "@/lib/sub-window"
// before the page: a review window asks for its reads before the chunk of its page
import "@/lib/review-window-start"
import { App } from "@/App"
import { logError } from "@/lib/errors"
import { loadWindowPage } from "@/router"

const rootElement = document.getElementById("root")
if (rootElement == null) {
  throw new Error("Root element #root not found")
}

const root = createRoot(rootElement)
// the first render has the page, so no frame of the window is without it
loadWindowPage(window.location.pathname).then(
  page =>
    root.render(
      <StrictMode>
        <App page={page} />
      </StrictMode>,
    ),
  // a path no page has, or a chunk that did not load: the window says so, as the 404 of a router would
  (error: unknown) => {
    logError("Cannot open the page of the window", error)
    root.render(<p className="p-4 text-sm text-destructive">Cannot open the page of the window: {error instanceof Error ? error.message : String(error)}</p>)
  },
)
