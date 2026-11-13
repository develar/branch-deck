// first: an error of a module after it, of a handler or of a promise nobody awaits reaches the log of the app
import "@/lib/page-errors"
import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import "./index.css"
import "@/lib/sub-window"
// before the page: a review window asks for its reads before the chunk of its page
import "@/lib/review-window-start"
import { App } from "@/App"
import { PageFailure } from "@/components/shared/PageFailure"
import { logUncaughtError } from "@/lib/page-errors"
import { markWindowContentReady, markWindowShellDrawn } from "@/lib/sub-window"
import { loadWindowPage } from "@/router"

const rootElement = document.getElementById("root")
if (rootElement == null) {
  throw new Error("Root element #root not found")
}

/** A render threw an error that nothing caught: React unmounted the page, which would leave a blank window */
const RENDER_FAILURE = "Cannot show the window"
/** A path no page has, or a chunk that did not load, as the 404 of a router */
const LOAD_FAILURE = "Cannot open the page of the window"

const root = createRoot(rootElement, {
  onUncaughtError: (error, errorInfo) => {
    logUncaughtError(RENDER_FAILURE, error, errorInfo.componentStack)
    showFailure(RENDER_FAILURE, error)
  },
})

let failed = false

/**
 * The window says that its page failed instead of the page (`PageFailure`), and shows at once: a window built hidden
 * would show a blank canvas by the deadline of the stage its page reached. Once: a failure of the failure is only logged
 */
function showFailure(title: string, error: unknown) {
  if (failed) {
    return
  }
  failed = true
  // after the commit that unmounted the page
  queueMicrotask(() => {
    root.render(<PageFailure title={title} error={error} />)
    markWindowShellDrawn()
    markWindowContentReady()
  })
}

// the first render has the page, so no frame of the window is without it
loadWindowPage(window.location.pathname).then(
  page =>
    root.render(
      <StrictMode>
        <App page={page} />
      </StrictMode>,
    ),
  (error: unknown) => {
    logUncaughtError(LOAD_FAILURE, error)
    showFailure(LOAD_FAILURE, error)
  },
)
