/**
 * The theme of the IDE in the page (docs/design/ide-embedding.md, "Implementation notes"): the host context of MCP Apps
 * names the theme (`light`, `dark`) and the style variables of SEP-1865 (`styles.variables`, and the extension
 * `com.jetbrains.air/variables` of Air with the accent). The page keeps its own semantic tokens (`src/index.css`) and
 * reads the IDE's through them: each variable of the host is set on the root under `--air-host-<name>` (the host's
 * `--font-sans` is no font of the page), and the stylesheet below points the tokens at them while the page shows the
 * theme of the IDE. The theme setting System follows the IDE: the page's query of the system's dark appearance answers
 * the theme of the host, and tells its change as the system's would.
 */

/** The part of the host context of MCP Apps the theme reads */
export interface HostTheme {
  readonly theme?: "light" | "dark"
  readonly styles?: { readonly variables?: Readonly<Record<string, string | undefined>> }
  readonly "com.jetbrains.air/variables"?: Readonly<Record<string, string | undefined>>
}

const SYSTEM_DARK = "(prefers-color-scheme: dark)"

/** The prefix of a variable of the host on the root of the page */
export const HOST_VARIABLE_PREFIX = "--air-host-"

/** The token of the page, the variable of the host it reads */
const TOKENS: readonly (readonly [string, string])[] = [
  ["--background", "--color-background-primary"],
  ["--foreground", "--color-text-primary"],
  ["--card", "--color-background-primary"],
  ["--card-foreground", "--color-text-primary"],
  ["--popover", "--color-background-secondary"],
  ["--popover-foreground", "--color-text-primary"],
  // the canvas is the frame behind the islands: the IDE's second surface
  ["--canvas", "--color-background-secondary"],
  ["--primary", "--air-color-accent"],
  ["--primary-foreground", "--air-color-accent-foreground"],
  ["--muted-foreground", "--color-text-secondary"],
  ["--destructive", "--color-text-danger"],
  ["--success", "--color-text-success"],
  ["--warning", "--color-text-warning"],
  ["--info", "--color-text-info"],
  ["--border", "--color-border-primary"],
  ["--input", "--color-border-secondary"],
  ["--ring", "--color-ring-primary"],
  ["--radius", "--border-radius-md"],
]

/**
 * The tokens over the variables the host gave, while the page shows the theme the host names (`data-air-theme`); a
 * token whose variable the host lacks keeps the page's
 */
function stylesheet(given: ReadonlySet<string>): string {
  const declarations = TOKENS.filter(([, variable]) => given.has(variable)).map(([token, variable]) => `  ${token}: var(${HOST_VARIABLE_PREFIX}${variable.slice(2)});`)
  return `:root[data-air-theme="light"][data-theme="light"], :root[data-air-theme="dark"][data-theme="dark"] {\n${declarations.join("\n")}\n}`
}

/** The theme of the host on the root of a page and the query of the system's appearance that answers it */
export class AirTheme {
  private dark = false
  private readonly listeners = new Set<(event: MediaQueryListEvent) => void>()
  private readonly given = new Set<string>()
  private readonly style: HTMLStyleElement

  constructor(private readonly target: Window) {
    this.style = target.document.createElement("style")
    this.style.dataset.air = "theme"
    target.document.head.append(this.style)
    this.answerSystemQuery()
  }

  /** Applies a host context, the first one or a change of it (`ui/notifications/host-context-changed`) */
  apply(context: HostTheme): void {
    const root = this.target.document.documentElement
    const variables = { ...context.styles?.variables, ...context["com.jetbrains.air/variables"] }
    for (const [name, value] of Object.entries(variables)) {
      const property = `${HOST_VARIABLE_PREFIX}${name.replace(/^--/, "")}`
      if (value === undefined || value === "") {
        root.style.removeProperty(property)
        this.given.delete(name)
      } else {
        root.style.setProperty(property, value)
        this.given.add(name)
      }
    }
    this.style.textContent = stylesheet(this.given)
    if (context.theme === undefined) {
      return
    }
    root.dataset.airTheme = context.theme
    const dark = context.theme === "dark"
    if (dark !== this.dark) {
      this.dark = dark
      for (const listener of this.listeners) {
        listener({ matches: dark, media: SYSTEM_DARK } as MediaQueryListEvent)
      }
    }
  }

  /** `matchMedia` of the system's dark appearance answers the theme of the IDE; every other query is the browser's */
  private answerSystemQuery(): void {
    const matchMedia = this.target.matchMedia.bind(this.target)
    const isDark = () => this.dark
    const listeners = this.listeners
    this.target.matchMedia = (query: string): MediaQueryList => {
      if (query.replace(/\s+/g, " ").trim() !== SYSTEM_DARK) {
        return matchMedia(query)
      }
      const list = {
        get matches() {
          return isDark()
        },
        media: SYSTEM_DARK,
        onchange: null,
        addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => listeners.add(listener),
        removeEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => listeners.delete(listener),
        addListener: (listener: (event: MediaQueryListEvent) => void) => listeners.add(listener),
        removeListener: (listener: (event: MediaQueryListEvent) => void) => listeners.delete(listener),
        dispatchEvent: () => false,
      } as unknown as MediaQueryList
      return list
    }
  }
}
