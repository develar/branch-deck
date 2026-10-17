import { useEffect } from "react"
import type { ComponentProps, ReactNode } from "react"
import { cn } from "cn"
import { WindowControls } from "@/components/repository/WindowControls"
import { useWindowFrameState } from "@/hooks/useWindowFrameState"
import { drawsTitleBar, titleBar } from "@/lib/title-bar"
import { installTitleBarMouse } from "@/lib/title-bar-mouse"
import { useAppSettings } from "@/stores/settings/appSettings"

export interface WindowTitleBarProps extends Omit<ComponentProps<"header">, "children"> {
  /** Where the user is: the object of the window, its navigation */
  children: ReactNode
  /** The view of the window and the actions on its object */
  end?: ReactNode
}

/**
 * The top row of a window, on the canvas above the islands: the main window's toolbar and the row of every sub-window.
 * Where the page draws the title bar (`titleBar`) it is the title bar: its empty parts drag the window, a double click
 * and a right click do what the system's title bar does there (`installTitleBarMouse`), on macOS it starts after the
 * traffic lights (not in fullscreen, where they are hidden), on Windows and Linux it has the window buttons at the
 * start and the end the desktop puts them. Where the system title bar stays it is a row below it.
 */
export function WindowTitleBar({ children, end, className, style, ...props }: WindowTitleBarProps) {
  const frame = useWindowFrameState()
  const zoom = useAppSettings(state => state.zoom)
  useEffect(() => installTitleBarMouse(), [])
  // the page rounds the corners of the window as the desktop's are (`cornerRadius`): square while the window fills the
  // screen or an edge of it, as GNOME's windows are
  const cornerRadius = titleBar.kind === "custom" && !frame.maximized && !frame.fullscreen && !frame.tiled ? titleBar.cornerRadius : 0
  useEffect(() => {
    const root = document.documentElement
    if (cornerRadius > 0) {
      root.dataset.windowRounded = ""
      root.style.setProperty("--window-radius", `${cornerRadius}px`)
    } else {
      delete root.dataset.windowRounded
    }
  }, [cornerRadius])
  const controls = titleBar.kind === "custom" ? titleBar.controls : null
  const hasStart = controls !== null && controls.start.length > 0
  const hasEnd = controls !== null && controls.end.length > 0
  return (
    <header
      className={cn(
        // a container: its width at its own size, not the viewport of the zoomed page, hides the secondary texts
        "@container flex h-10 shrink-0 items-center gap-4 select-none",
        hasStart ? "pl-0" : titleBar.kind === "overlay" && !frame.fullscreen ? "pl-22" : "pl-2",
        hasEnd ? "pr-0" : "pr-2",
        className,
      )}
      data-title-bar={titleBar.kind}
      data-tauri-drag-region={drawsTitleBar ? "deep" : undefined}
      // the buttons of an inactive window dim (`in-data-window-inactive`)
      data-window-inactive={drawsTitleBar && !frame.active ? "" : undefined}
      // the title bar keeps the size of the window chrome, the page zoom (`useZoom`) leaves it alone
      style={zoom === 1 ? style : { ...style, zoom: 1 / zoom }}
      {...props}
    >
      {controls !== null && <WindowControls buttons={controls.start} style={controls.style} frame={frame} side="start" />}
      {/* the expanded main menu (☰) lies over the rest of the start, which hides meanwhile */}
      <div className="flex min-w-0 flex-1 items-center gap-3 has-data-[main-menu=expanded]:*:not-data-main-menu:invisible">{children}</div>
      {end !== undefined && <div className="flex shrink-0 items-center gap-3">{end}</div>}
      {controls !== null && <WindowControls buttons={controls.end} style={controls.style} frame={frame} side="end" />}
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
