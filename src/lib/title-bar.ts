/**
 * How every window draws its title bar, set by the window builder (`src-tauri/src/title_bar.rs`) before the page loads;
 * the top row of the page (`WindowTitleBar`) is the title bar:
 * - `overlay` on macOS: beside the traffic lights;
 * - `custom` on Windows: no decorations, the row draws the window buttons;
 * - `null` on Linux and in E2E: the native title bar above the row.
 */
export type TitleBarKind = "overlay" | "custom"

export const titleBar: TitleBarKind | null = window.__BD_TITLE_BAR__ ?? null
