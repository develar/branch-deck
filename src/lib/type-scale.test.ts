import { describe, expect, it } from "vite-plus/test"
import { cn } from "cn"

// the custom steps of the type scale in src/index.css (docs/design/ui-theme.md, "Type sizes")
describe("type scale with cn", () => {
  it("merges text-md as a font size: it replaces text-sm and keeps the colour", () => {
    expect(cn("text-sm text-foreground", "text-md")).toBe("text-foreground text-md")
  })

  it("keeps the size of a Windows caption glyph next to the text colour", () => {
    expect(cn("font-window-controls bd-window-control-glyph text-foreground")).toBe("font-window-controls bd-window-control-glyph text-foreground")
  })
})
