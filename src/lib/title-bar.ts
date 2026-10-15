/**
 * How the main window draws its title bar, set by the window builder (`src-tauri/src/lib.rs`) before the page loads:
 * - `overlay` on macOS: the toolbar is the title bar, beside the traffic lights;
 * - `custom` on Windows: no decorations, the toolbar is the title bar and draws the window buttons;
 * - `null` on Linux, in the sub-windows and in E2E: the native title bar above the toolbar.
 */
export type TitleBarKind = "overlay" | "custom"

export const titleBar: TitleBarKind | null = window.__BD_TITLE_BAR__ ?? null
