import type { ComponentType } from "react"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { LinkedText } from "@/components/commit/LinkedText"
import { IssueHoverHost } from "@/components/issue/IssueHoverHost"
import { useMainMenuKeys } from "@/hooks/useMainMenuKeys"
import { useTheme } from "@/hooks/useTheme"
import { useZoom } from "@/hooks/useZoom"
import { useAuthSync } from "@/stores/auth"

/**
 * The root of every window: its page (by the path of its URL, `src/router.ts`), the tooltips, the toasts, the issue
 * popover, the sign-ins, the zoom, the theme and the keys of the main menu.
 */
export function App({ page: Page }: { page: ComponentType }) {
  useZoom()
  useTheme()
  // every window knows the sign-ins: whether the links of the YouTrack issues show the issue, what GitHub gives
  useAuthSync()
  // the keys of the main menu where it replaces the native menu (a custom title bar on Windows and Linux)
  useMainMenuKeys()

  return (
    <TooltipProvider>
      <Page />
      <Toaster renderText={renderToastText} />
      <IssueHoverHost />
    </TooltipProvider>
  )
}

/** The issue keys of a toast are links, as in the rows; in the title a key keeps its colour, as in a branch name */
function renderToastText(text: string, part: "title" | "description") {
  return <LinkedText text={text} linkClassName={part === "title" ? "text-foreground hover:underline" : undefined} />
}
