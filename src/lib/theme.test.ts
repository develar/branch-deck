// @spec spec/repository/settings.spec.md
import { describe, expect, it } from "vite-plus/test"
import { shownTheme } from "@/lib/theme"

describe("shownTheme", () => {
  it("follows the system for System, and keeps Light and Dark whatever the system shows", () => {
    expect(shownTheme("system", true)).toBe("dark")
    expect(shownTheme("system", false)).toBe("light")
    expect(shownTheme("light", true)).toBe("light")
    expect(shownTheme("dark", false)).toBe("dark")
  })
})
