// The entry of the page in a view of the IDE (`vite build --mode air`, docs/design/ide-embedding.md): the frame link,
// `bd_bootstrap` and the internals of Tauri first, then the main of the desktop app, whose modules read their globals as
// they load
import { logError } from "@/lib/errors"
import { startAirPage } from "@/platform/air/start"

startAirPage(window).then(
  () => import("@/main"),
  (error: unknown) => {
    logError("Cannot start Branch Deck in the IDE", error)
    const message = document.createElement("p")
    message.textContent = `Cannot start Branch Deck: ${error instanceof Error ? error.message : String(error)}`
    document.getElementById("root")?.append(message)
  },
)
