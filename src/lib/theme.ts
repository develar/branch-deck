import colors from "tailwindcss/colors"

type TailwindColors = typeof colors

const NON_PRIMARY_COLOR_NAMES = new Set<string>([
  "slate",
  "gray",
  "zinc",
  "neutral",
  "stone",
  "black",
  "white",
  "transparent",
  "current",
  "inherit",
])

const SHADES = ["50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"] as const

type Palette = Record<(typeof SHADES)[number], string>

function isPalette(value: TailwindColors[keyof TailwindColors]): value is Palette {
  return typeof value === "object" && value !== null && "500" in value
}

/** Tailwind palette names that can be used as the primary colour (neutrals excluded). */
export const PRIMARY_COLOR_NAMES: readonly string[] = Object.keys(colors)
  .filter(name => !NON_PRIMARY_COLOR_NAMES.has(name) && isPalette(colors[name as keyof TailwindColors]))

export const DEFAULT_PRIMARY_COLOR = "green"

/**
 * Sets the `--bd-primary-*` CSS variables on `<html>` from the given Tailwind palette.
 * `--primary` and `--ring` reference these variables, so the change applies immediately.
 * Unknown names fall back to the default primary colour.
 */
export function applyPrimaryColor(name: string): void {
  const candidate = PRIMARY_COLOR_NAMES.includes(name) ? colors[name as keyof TailwindColors] : undefined
  const fallback = colors[DEFAULT_PRIMARY_COLOR]
  const palette = candidate !== undefined && isPalette(candidate) ? candidate : fallback
  const style = document.documentElement.style
  for (const shade of SHADES) {
    style.setProperty(`--bd-primary-${shade}`, palette[shade])
  }
}
