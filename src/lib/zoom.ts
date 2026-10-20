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

/** The title bar of a sub-window fits one line from 800 px; the sidebar and the diffs of a review need 640 CSS px */
const MIN_SUB_WINDOW = { width: 800, height: 500, islandsWidth: 640 } as const
/** The sidebar of Settings beside a page fits from 600 × 420 px; the page needs 560 CSS px */
const MIN_SETTINGS_WINDOW = { width: 600, height: 420, islandsWidth: 560 } as const
/** The search above the entries of Activity fits from 440 × 400 px; the entries need 400 CSS px */
const MIN_ACTIVITY_WINDOW = { width: 440, height: 400, islandsWidth: 400 } as const

/**
 * The minimum size of the sub-window `label` at `zoom`, as `sub_window_min_size` in `src-tauri/src/zoom.rs`: a zoom
 * that needs more than the window's own minimum for its islands widens it (above 125% for a review, 107% for Settings,
 * 110% for Activity)
 */
export function minSubWindowSize(label: string, zoom: number): { width: number; height: number } {
  const min = label === "settings" ? MIN_SETTINGS_WINDOW : label === "activity" ? MIN_ACTIVITY_WINDOW : MIN_SUB_WINDOW
  return { width: Math.max(min.width, min.islandsWidth * zoom), height: min.height }
}
