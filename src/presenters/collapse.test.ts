import { describe, expect, it } from "vite-plus/test"
import { formatCollapse } from "@/presenters/collapse"

describe("formatCollapse", () => {
  it("names the generated file by its name, and leaves a file without text and a large file to their notice and counts", () => {
    expect(formatCollapse({ kind: "generated", name: "lockFile" })).toBe("Lock file")
    expect(formatCollapse({ kind: "generated", name: "minified" })).toBe("Minified")
    expect(formatCollapse({ kind: "generated", name: "sourceMap" })).toBe("Source map")
    expect(formatCollapse({ kind: "generated", name: "snapshot" })).toBe("Snapshot")
    expect(formatCollapse({ kind: "noText" })).toBeNull()
    expect(formatCollapse({ kind: "large", changed: 1240 })).toBeNull()
    expect(formatCollapse(null)).toBeNull()
  })
})
