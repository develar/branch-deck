import { getCurrentWindow } from "@tauri-apps/api/window"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { logUncaughtError } from "@/lib/page-errors"
import { drawsTitleBar } from "@/lib/title-bar"

/**
 * What a window shows when its page failed (openspec/specs/repository/title-bar/spec.md, "A window whose page fails says
 * so"): a render threw an error that nothing caught, or the page did not load. It says the error, which the log has with
 * its stack, and offers Reload and Close Window: where the page draws the title bar, the window has no other buttons, so
 * its top row only drags it. It uses nothing of the page that may have failed: no store, no title row, no toast.
 */
export function PageFailure({ title, error }: { title: string; error: unknown }) {
  const message = error instanceof Error ? error.message : String(error)
  return (
    <div className="flex h-screen flex-col">
      <div className="h-10 shrink-0" data-testid="page-failure-row" data-tauri-drag-region={drawsTitleBar ? "deep" : undefined} />
      <div className="flex min-h-0 flex-1 justify-center overflow-y-auto px-4 pb-4">
        <Card className="h-fit w-full max-w-xl" data-testid="page-failure">
          <CardHeader className="border-b">
            <h3 className="text-base font-semibold">{title}</h3>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <p className="font-mono text-xs break-words whitespace-pre-wrap text-destructive">{message}</p>
            <p className="text-muted-foreground">The log of Branch Deck has the details. Reload the window to try again.</p>
            <div className="flex gap-2">
              <Button onClick={() => window.location.reload()}>Reload</Button>
              <Button variant="outline" onClick={closeWindow}>
                Close Window
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

function closeWindow() {
  getCurrentWindow()
    .close()
    .catch((error: unknown) => logUncaughtError("Cannot close the window", error))
}
