/** The zoom levels of the pages, those of Chromium from 67% to 150% */
export const ZOOM_LEVELS = [0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5] as const

/** The menu items Actual Size, Zoom In and Zoom Out (`src-tauri/src/menu.rs`) */
export type ZoomAction = "reset" | "in" | "out"

/** The level after `action`; a level that is not one of `ZOOM_LEVELS` steps to the next one in that direction */
export function nextZoom(current: number, action: ZoomAction): number {
  switch (action) {
    case "reset":
      return 1
    case "in":
      return ZOOM_LEVELS.find(level => level > current + 0.001) ?? Math.max(...ZOOM_LEVELS)
    case "out":
      return ZOOM_LEVELS.findLast(level => level < current - 0.001) ?? Math.min(...ZOOM_LEVELS)
  }
}

/** The minimum size of the main window at 100% (`src-tauri/src/zoom.rs`): the header fits one line from 960 px */
export const MIN_WINDOW = { width: 960, height: 600 } as const
/** The islands need 768 CSS px (`md`), so a zoom above 125% widens the minimum window */
const MIN_ISLANDS_WIDTH = 768

/** The minimum width of the main window at `zoom`, as `min_window_width` in `src-tauri/src/zoom.rs` */
export function minWindowWidth(zoom: number): number {
  return Math.max(MIN_WINDOW.width, MIN_ISLANDS_WIDTH * zoom)
}

/** The minimum size of a sub-window at 100% (`src-tauri/src/zoom.rs`): its title bar fits one line from 800 px */
export const MIN_SUB_WINDOW = { width: 800, height: 500 } as const
/** The sidebar and the diffs of a review need 640 CSS px, so a zoom above 125% widens the minimum sub-window */
const MIN_SUB_WINDOW_ISLANDS_WIDTH = 640

/** The minimum width of a sub-window at `zoom`, as `min_sub_window_width` in `src-tauri/src/zoom.rs` */
export function minSubWindowWidth(zoom: number): number {
  return Math.max(MIN_SUB_WINDOW.width, MIN_SUB_WINDOW_ISLANDS_WIDTH * zoom)
}
