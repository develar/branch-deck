import { getCurrentWindow } from "@tauri-apps/api/window"
import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { notifyError } from "@/lib/errors"

/** The glyphs of the Windows 11 caption buttons (Segoe Fluent Icons, Segoe MDL2 Assets on Windows 10) */
const MINIMIZE = ""
const MAXIMIZE = ""
const RESTORE = ""
const CLOSE = ""

/**
 * The window buttons of a window without decorations (Windows, `titleBar === "custom"`), at the right end of the
 * toolbar: 46×40 like the native caption buttons, Close turning red on hover.
 */
export function WindowControls({ maximized }: { maximized: boolean }) {
  const run = (action: string, call: () => Promise<void>) => () => {
    call().catch((error: unknown) => {
      notifyError(`Cannot ${action} the window`, error)
    })
  }
  return (
    <div className="flex self-stretch" data-testid="window-controls">
      <WindowControl label="Minimize" glyph={MINIMIZE} onClick={run("minimize", () => getCurrentWindow().minimize())} />
      <WindowControl
        label={maximized ? "Restore" : "Maximize"}
        glyph={maximized ? RESTORE : MAXIMIZE}
        onClick={run(maximized ? "restore" : "maximize", () => getCurrentWindow().toggleMaximize())}
      />
      <WindowControl
        label="Close"
        glyph={CLOSE}
        className="hover:bg-destructive hover:text-destructive-foreground"
        onClick={run("close", () => getCurrentWindow().close())}
      />
    </div>
  )
}

function WindowControl({ label, glyph, className, onClick }: { label: string, glyph: string, className?: string, onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <button
            type="button"
            aria-label={label}
            className={cn(
              "flex w-11.5 items-center justify-center font-window-controls bd-window-control-glyph text-foreground outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:ring-inset",
              className,
            )}
            onClick={onClick}
          />
        )}
      >
        <span aria-hidden>{glyph}</span>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  )
}
