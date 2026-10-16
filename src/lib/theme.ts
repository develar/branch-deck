import colors from "tailwindcss/colors"

type TailwindColors = typeof colors

const NON_PRIMARY_COLOR_NAMES = new Set<string>([
  "slate",
  "gray",
  "zinc",
  "neutral",
  "stone",
  // the neutrals added in Tailwind 4.3
  "mauve",
  "olive",
  "mist",
  "taupe",
  "black",
  "white",
  "transparent",
  "current",
  "inherit",
])

const SHADES = ["50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"] as const

type Shades = Record<(typeof SHADES)[number], string>

/** A primary colour of the "Theme…" dialog. `src/index.css` takes shade 600 in the light theme and 400 in the dark one. */
export interface PrimaryPalette {
  name: string
  label: string
  shades: Shades
}

/**
 * The accent of the IntelliJ Islands themes: the `blue-160` … `blue-40` ramp of `ManyIslandsLight.theme.json`, so that
 * 600 is `Button.default` (`blue-80`) and 400 is the dark-theme link (`blue-100`).
 */
const INTELLIJ_PALETTE: PrimaryPalette = {
  name: "intellij",
  label: "IntelliJ",
  shades: {
    50: "#F7F8FF",
    100: "#E3EBFE",
    200: "#D0DFFE",
    300: "#A7C5FF",
    400: "#71A1FE",
    500: "#538AF9",
    600: "#3871E1",
    700: "#2F5EB9",
    800: "#2E4D89",
    900: "#2A4371",
    950: "#233558",
  },
}

function isShades(value: TailwindColors[keyof TailwindColors]): value is Shades {
  return typeof value === "object" && value !== null && "500" in value
}

/** The primary colours offered in the "Theme…" dialog: the IntelliJ accent, then the Tailwind palettes (neutrals excluded). */
export const PRIMARY_PALETTES: readonly PrimaryPalette[] = [
  INTELLIJ_PALETTE,
  ...Object.entries(colors).flatMap(([name, value]) =>
    !NON_PRIMARY_COLOR_NAMES.has(name) && isShades(value) ? [{ name, label: name.charAt(0).toUpperCase() + name.slice(1), shades: value }] : [],
  ),
]

export const DEFAULT_PRIMARY_COLOR = INTELLIJ_PALETTE.name

/**
 * Sets the `--bd-primary-*` CSS variables on `<html>` from the given palette.
 * `--primary` and `--ring` reference these variables, so the change applies immediately.
 * Unknown names fall back to the default primary colour.
 */
export function applyPrimaryColor(name: string): void {
  const palette = PRIMARY_PALETTES.find(it => it.name === name) ?? INTELLIJ_PALETTE
  const style = document.documentElement.style
  for (const shade of SHADES) {
    style.setProperty(`--bd-primary-${shade}`, palette.shades[shade])
  }
}
