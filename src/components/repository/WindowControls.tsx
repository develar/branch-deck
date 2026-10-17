import type { ReactNode } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { WindowFrameState } from "@/hooks/useWindowFrameState"
import { notifyError } from "@/lib/errors"
import type { WindowButton, WindowButtonStyle } from "@/lib/title-bar"

/** The glyphs of the Windows 11 caption buttons (Segoe Fluent Icons, Segoe MDL2 Assets on Windows 10) */
// escapes, not the characters: the private-use code points of the font read as nothing and get lost in an edit
const WINDOWS_GLYPHS = { minimize: "\ue921", maximize: "\ue922", restore: "\ue923", close: "\ue8bb" } as const

/** IntelliJ's Linux window button icons (platform/icons/src/linux/theme), their strokes: GNOME 24×24, KDE 20×20 */
const GNOME_PATHS = {
  minimize: ["M8 13.5H16"],
  maximize: ["M8.5 8.5V15.5H15.5V8.5H8.5Z"],
  restore: ["M8.5 9.5V15.5H14.5V9.5H8.5Z", "M16.5 14V7.5H10"],
  close: ["M15.5 15.5L8.5 8.5M15.5 8.5L8.5 15.5"],
} as const
const KDE_PATHS = {
  minimize: ["M15 8L10 13L5 8"],
  maximize: ["M15 12L10 7L5 12"],
  restore: ["M14.5 10L10 5.5L5.5 10L10 14.5L14.5 10Z"],
  close: ["M14 14L6 6M14 6L6 14"],
} as const

type Glyph = keyof typeof WINDOWS_GLYPHS

export interface WindowControlsProps {
  /** In the order the desktop shows them (`gtk-decoration-layout`, `WindowControlsLayout`) */
  buttons: readonly WindowButton[]
  style: WindowButtonStyle
  frame: WindowFrameState
  /** At the start or the end of the title bar */
  side: "start" | "end"
}

/**
 * The window buttons of a custom title bar (`titleBar.kind === "custom"`), each one cell of the 40px row:
 * - `windows`: 46×40 like the native caption buttons, Segoe Fluent glyphs, Close turning red on hover;
 * - `gnome`: a 47×40 cell, IntelliJ's Adwaita icon: a glyph on a shade circle (none while the window is inactive);
 * - `kde`: a 47×40 cell, IntelliJ's Breeze icon: chevrons that get a filled circle on hover, Close always on one.
 * Maximize is Restore for a maximized or fullscreen window (which it leaves). They are not in the tab order, as native
 * window buttons aren't; their names and tooltips stay.
 */
export function WindowControls({ buttons, style, frame, side }: WindowControlsProps) {
  if (buttons.length === 0) {
    return null
  }
  const run = (action: string, call: () => Promise<void>) => () => {
    call().catch((error: unknown) => {
      notifyError(`Cannot ${action} the window`, error)
    })
  }
  const restores = frame.maximized || frame.fullscreen
  return (
    <div className="flex shrink-0 self-stretch" data-testid="window-controls" data-side={side} data-style={style}>
      {buttons.map(button => {
        switch (button) {
          case "minimize":
            return <WindowControl key={button} style={style} label="Minimize" glyph="minimize" onClick={run("minimize", () => getCurrentWindow().minimize())} />
          case "maximize":
            return (
              <WindowControl
                key={button}
                style={style}
                label={restores ? "Restore" : "Maximize"}
                glyph={restores ? "restore" : "maximize"}
                onClick={
                  frame.fullscreen
                    ? run("leave fullscreen", () => getCurrentWindow().setFullscreen(false))
                    : run(restores ? "restore" : "maximize", () => getCurrentWindow().toggleMaximize())
                }
              />
            )
          case "close":
            return <WindowControl key={button} style={style} label="Close" glyph="close" onClick={run("close", () => getCurrentWindow().close())} />
        }
      })}
    </div>
  )
}

function WindowControl({ style, label, glyph, onClick }: { style: WindowButtonStyle; label: string; glyph: Glyph; onClick: () => void }) {
  const isClose = glyph === "close"
  let icon: ReactNode
  let className: string
  switch (style) {
    case "windows":
      icon = (
        <span aria-hidden className="font-window-controls bd-window-control-glyph">
          {WINDOWS_GLYPHS[glyph]}
        </span>
      )
      className = cn("w-11.5 text-foreground hover:bg-muted in-data-window-inactive:text-muted-foreground", isClose && "hover:bg-destructive hover:text-destructive-foreground")
      break
    case "gnome":
      // the shade circle is drawn in the icon, as IntelliJ's: WebKitGTK doesn't round the background of an <svg>
      icon = (
        <svg aria-hidden viewBox="0 0 24 24" fill="none" className="size-6 text-window-button-glyph">
          <circle
            cx="12"
            cy="12"
            r="12"
            className="fill-window-button-shade/5 group-hover/control:fill-window-button-shade/8 group-active/control:fill-window-button-shade/10 in-data-window-inactive:fill-transparent"
          />
          {GNOME_PATHS[glyph].map(path => (
            <path key={path} d={path} stroke="currentColor" strokeLinecap={glyph === "minimize" ? undefined : "round"} />
          ))}
        </svg>
      )
      className = "w-11.75"
      break
    case "kde":
      icon = (
        <svg aria-hidden viewBox="0 0 20 20" fill="none" className="size-5">
          <circle
            cx="10"
            cy="10"
            r="9"
            className={cn(
              "fill-window-button-kde-circle group-hover/control:fill-window-button-kde-circle-hover group-active/control:fill-window-button-kde-circle-pressed",
              !isClose && "opacity-0 group-hover/control:opacity-100",
            )}
          />
          {KDE_PATHS[glyph].map(path => (
            <path
              key={path}
              d={path}
              strokeLinecap="round"
              strokeLinejoin="round"
              className={isClose ? "stroke-window-button-kde-on-circle" : "stroke-window-button-kde-glyph group-hover/control:stroke-window-button-kde-on-circle"}
            />
          ))}
        </svg>
      )
      className = "w-11.75"
      break
  }
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button type="button" aria-label={label} tabIndex={-1} className={cn("group/control flex items-center justify-center outline-none", className)} onClick={onClick} />
        }
      >
        {icon}
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  )
}
