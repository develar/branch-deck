import { describe, expect, it } from "vite-plus/test"
import indexCss from "@/index.css?raw"
import islandsCss from "@/islands.generated.css?raw"

// The contrast of the token pairs of src/index.css (docs/design/ui-theme.md, "Palette"), on the values of the
// --islands-* variables it maps: text at 4.5:1 and UI marks at 3:1 (WCAG 2.2, 1.4.3 and 1.4.11), on the island, on a
// popup and on the canvas, and a status text on the 10% tint of its colour (Callout, Badge, Alert)

interface Color {
  r: number
  g: number
  b: number
  a: number
}

type Scope = Map<string, string>

function declarations(block: string, scope: Scope = new Map()): Scope {
  for (const [, name, value] of block.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    scope.set(name!, value!.trim())
  }
  return scope
}

function themeScopes(): { light: Scope; dark: Scope } {
  const islands = declarations(islandsCss)
  // the top-level :root blocks; the dark block is the indented one in the media query
  const light = new Map(islands)
  for (const [, block] of indexCss.matchAll(/^:root\s*\{([^}]*)\}/gm)) {
    declarations(block!, light)
  }
  const darkBlock = /@media \(prefers-color-scheme: dark\)\s*\{\s*:root\s*\{([^}]*)\}/.exec(indexCss)?.[1]
  if (darkBlock === undefined) {
    throw new Error("src/index.css has no dark :root block")
  }
  return { light, dark: declarations(darkBlock, new Map(light)) }
}

function hex(value: string): Color {
  const channel = (index: number) => parseInt(value.slice(1 + index * 2, 3 + index * 2), 16) / 255
  return { r: channel(0), g: channel(1), b: channel(2), a: value.length === 9 ? channel(3) : 1 }
}

/** The colour of a token: a hex, a `var()` of another token, or a translucent `color-mix` with `transparent` */
function resolve(scope: Scope, value: string): Color {
  const reference = /^var\((--[\w-]+)\)$/.exec(value)
  if (reference !== null) {
    const target = scope.get(reference[1]!)
    if (target === undefined) {
      throw new Error(`${reference[1]} is not declared`)
    }
    return resolve(scope, target)
  }
  if (/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(value)) {
    return hex(value)
  }
  const mix = /^color-mix\(in \w+, (.+) (\S+), transparent\)$/.exec(value)
  if (mix !== null) {
    const percent = mix[2]!.startsWith("var(") ? scope.get(mix[2]!.slice(4, -1)) : mix[2]
    const color = resolve(scope, mix[1]!)
    return { ...color, a: color.a * (parseFloat(percent ?? "NaN") / 100) }
  }
  throw new Error(`no colour of the form the test reads: ${value}`)
}

/** `top` painted over the opaque `bottom` */
function over(top: Color, bottom: Color): Color {
  const mix = (channel: "r" | "g" | "b") => top[channel] * top.a + bottom[channel] * (1 - top.a)
  return { r: mix("r"), g: mix("g"), b: mix("b"), a: 1 }
}

function luminance({ r, g, b }: Color): number {
  const linear = (channel: number) => (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4)
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/** The ratio of two opaque colours */
function contrast(text: Color, surface: Color): number {
  if (text.a !== 1 || surface.a !== 1) {
    throw new Error("a translucent colour has no contrast before it is painted over a surface")
  }
  const [lighter, darker] = [luminance(text), luminance(surface)].sort((x, y) => y - x)
  return (lighter! + 0.05) / (darker! + 0.05)
}

const TEXT = 4.5
const MARK = 3
const SURFACES = ["card", "popover", "canvas"]
const STATUSES = ["destructive", "success", "warning", "info"]

/** The colour of a token of the theme under test */
type Token = (name: string) => Color

interface Pair {
  name: string
  min: number
  ratio: (token: Token) => number
}

const PAIRS: Pair[] = [
  ...["foreground", "muted-foreground", "primary", ...STATUSES].flatMap(text =>
    SURFACES.map(surface => ({ name: `${text} on ${surface}`, min: TEXT, ratio: (token: Token) => contrast(token(text), token(surface)) })),
  ),
  // a row hover, a chip, a code box: the tint of the foreground on the island
  ...["foreground", "muted-foreground"].map(text => ({
    name: `${text} on muted over card`,
    min: TEXT,
    ratio: (token: Token) => contrast(token(text), over(token("muted"), token("card"))),
  })),
  // a Callout, a Badge, an Alert, a status button: the colour on its own 10% tint, in an island and in a popup
  ...["primary", ...STATUSES].flatMap(text =>
    ["card", "popover"].map(surface => ({
      name: `${text} on its 10% tint over ${surface}`,
      min: TEXT,
      ratio: (token: Token) => {
        const surfaceColor = token(surface)
        const tinted = over({ ...token(text), a: 0.1 }, surfaceColor)
        return contrast(over(token(text), tinted), tinted)
      },
    })),
  ),
  ...["primary", ...STATUSES].map(fill => ({
    name: `${fill}-foreground on ${fill}`,
    min: TEXT,
    ratio: (token: Token) => contrast(token(`${fill}-foreground`), token(fill)),
  })),
  // the selection, the robot line, the focus ring
  ...["card", "canvas"].map(surface => ({
    name: `primary as a mark on ${surface}`,
    min: MARK,
    ratio: (token: Token) => contrast(token("primary"), token(surface)),
  })),
]

const scopes = themeScopes()

describe.each(["light", "dark"] as const)("the %s tokens", theme => {
  const scope = scopes[theme]
  const token: Token = name => {
    const value = scope.get(`--${name}`)
    if (value === undefined) {
      throw new Error(`--${name} is not declared`)
    }
    return resolve(scope, value)
  }

  it.each(PAIRS)("$name", ({ name, min, ratio }) => {
    const measured = ratio(token)
    expect(measured, `${theme}: ${name} is ${measured.toFixed(2)}:1`).toBeGreaterThanOrEqual(min)
  })

  it("reads every token from the Islands variables, not from a literal", () => {
    const literals = ["canvas", "background", "card", "popover", "foreground", "muted-foreground", "primary", "border", "input", ...STATUSES]
      .map(name => `--${name}: ${scope.get(`--${name}`)}`)
      .filter(line => !line.includes("var(--islands-"))
    expect(literals).toEqual([])
  })
})
