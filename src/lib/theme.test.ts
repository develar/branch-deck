import { describe, expect, it } from "vite-plus/test"
import { DEFAULT_PRIMARY_COLOR, PRIMARY_PALETTES } from "@/lib/theme"

describe("primary palettes", () => {
  it("should offer the IntelliJ accent first, as the default", () => {
    expect(PRIMARY_PALETTES[0]).toMatchObject({ name: "intellij", label: "IntelliJ" })
    expect(DEFAULT_PRIMARY_COLOR).toBe("intellij")
    // the light theme takes 600 (Button.default), the dark one 400 (the dark-theme link)
    expect(PRIMARY_PALETTES[0]?.shades["600"]).toBe("#3871E1")
    expect(PRIMARY_PALETTES[0]?.shades["400"]).toBe("#71A1FE")
  })

  it("should not offer the neutrals", () => {
    const names = PRIMARY_PALETTES.map(it => it.name)
    for (const neutral of ["slate", "gray", "zinc", "neutral", "stone", "mauve", "olive", "mist", "taupe"]) {
      expect(names).not.toContain(neutral)
    }
    expect(names).toContain("green")
    expect(PRIMARY_PALETTES.find(it => it.name === "green")?.label).toBe("Green")
  })

  it("should give every palette all the shades", () => {
    for (const palette of PRIMARY_PALETTES) {
      expect(Object.keys(palette.shades).sort()).toEqual(["100", "200", "300", "400", "50", "500", "600", "700", "800", "900", "950"])
    }
  })
})
