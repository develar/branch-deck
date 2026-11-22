/**
 * The title bar of a window of the IDE that merges the top row of the page into its own (openspec/specs/repository/
 * ide-embedding/spec.md, "Page: The IDE moves a window whose row is in its title bar"): the host context names it
 * (`hostContext["x-air-app"].titleBar`), and the IDE moves the window and does the double click of the system. The page
 * has no drag script of Tauri: a press on the empty parts of the row tells the IDE (`x-air-app/windowDrag`).
 */
import { isOnDragRegion } from "@/lib/drag-region"

/**
 * The title bar of the place of the view, as the IDE names it: `native` where the IDE draws its own title (a tool
 * window, an editor tab, a dialog, any place without the key); `overlay` where the row of the page is in the title bar
 * of the window, `height` px high (1 to 200), its content from `insetStart` px (after the traffic lights, 0 in full
 * screen)
 */
export type HostTitleBar = { readonly kind: "native" } | { readonly kind: "overlay"; readonly height: number; readonly insetStart: number }

export const NATIVE_TITLE_BAR: HostTitleBar = { kind: "native" }

/** The title bar a host context names: an overlay one with its height and its inset, else `native` */
export function hostTitleBarOf(value: unknown): HostTitleBar {
  const titleBar = value as { kind?: unknown; height?: unknown; insetStart?: unknown } | null | undefined
  if (titleBar?.kind !== "overlay" || typeof titleBar.height !== "number" || typeof titleBar.insetStart !== "number") {
    return NATIVE_TITLE_BAR
  }
  return { kind: "overlay", height: titleBar.height, insetStart: titleBar.insetStart }
}

/** The window fills the screen: the IDE hides the traffic lights, so the row starts at the edge */
export function isFullScreen(titleBar: HostTitleBar): boolean {
  return titleBar.kind === "overlay" && titleBar.insetStart === 0
}

/**
 * Tells `drag` of each left press on the empty parts of the row (`isOnDragRegion`) at its mouse down, before the page gets
 * it, with the count of its clicks (`event.detail`). The IDE takes it only for the press that runs now, with the same
 * count: 2 does the double click of the system, and any other count moves the window
 */
export function installWindowDrag(target: Window, drag: (clickCount: number) => void): void {
  target.addEventListener(
    "mousedown",
    event => {
      if (event.button !== 0 || !isOnDragRegion(event.composedPath())) {
        return
      }
      // no text selection and no focus change: the press moves the window
      event.preventDefault()
      drag(Math.max(event.detail, 1))
    },
    { capture: true },
  )
}
