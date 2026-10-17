import type { ComponentProps, ReactNode } from "react"
import { cn } from "cn"
import { WindowControls } from "@/components/repository/WindowControls"
import { useWindowFrameState } from "@/hooks/useWindowFrameState"
import { titleBar } from "@/lib/title-bar"
import { useAppSettings } from "@/stores/settings/appSettings"

export interface WindowTitleBarProps extends Omit<ComponentProps<"header">, "children"> {
  /** Where the user is: the object of the window, its navigation */
  children: ReactNode
  /** The view of the window and the actions on its object */
  end?: ReactNode
}

/**
 * The top row of a window, on the canvas above the islands: the main window's toolbar and the row of every sub-window.
 * Where the page draws the title bar (`titleBar`) it is the title bar: its empty parts drag the window, on macOS it
 * starts after the traffic lights (not in fullscreen, where they are hidden), on Windows it ends with the window
 * buttons. On Linux it is a row below the native title bar.
 */
export function WindowTitleBar({ children, end, className, style, ...props }: WindowTitleBarProps) {
  const frame = useWindowFrameState()
  const zoom = useAppSettings(state => state.zoom)
  return (
    <header
      className={cn(
        // a container: its width at its own size, not the viewport of the zoomed page, hides the secondary texts
        "@container flex h-10 shrink-0 items-center gap-4 select-none",
        titleBar === "overlay" && !frame.fullscreen ? "pl-22" : "pl-2",
        titleBar === "custom" ? "pr-0" : "pr-2",
        className,
      )}
      data-title-bar={titleBar ?? "native"}
      data-tauri-drag-region={titleBar === null ? undefined : "deep"}
      // the title bar keeps the size of the window chrome, the page zoom (`useZoom`) leaves it alone
      style={zoom === 1 ? style : { ...style, zoom: 1 / zoom }}
      {...props}
    >
      <div className="flex min-w-0 flex-1 items-center gap-3">{children}</div>
      {end !== undefined && <div className="flex shrink-0 items-center gap-3">{end}</div>}
      {titleBar === "custom" && <WindowControls maximized={frame.maximized} />}
    </header>
  )
}

/** "ui › Conflicting files": the parts of the place a window shows, the last one in the foreground */
export function WindowTitle({ parts, className }: { parts: readonly string[]; className?: string }) {
  return (
    <h1 className={cn("flex min-w-0 items-center gap-1.5 text-sm font-normal", className)} data-testid="window-title">
      {parts.map((part, index) => (
        <span key={index} className={cn("truncate", index === parts.length - 1 ? "shrink-0 font-medium text-foreground" : "text-muted-foreground")}>
          {index > 0 && <span className="mr-1.5 text-muted-foreground">›</span>}
          {part}
        </span>
      ))}
    </h1>
  )
}
