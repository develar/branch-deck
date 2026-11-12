import { describe, expect, it } from "vite-plus/test"
import { formatAlsoIn, formatConcernEvidence, formatConcernName, formatHome, presentConcernTitle } from "@/presenters/review-concerns"

describe("the words of the concerns", () => {
  it("names a concern by its evidence, and says why its parts are together", () => {
    expect(formatConcernName({ source: "token", token: "proxy" })).toBe("proxy")
    expect(formatConcernName({ source: "replacement", removed: "grep", added: "accio" })).toBe("grep → accio")
    expect(formatConcernName({ source: "declaration", name: "open" })).toBe("open")
    expect(formatConcernName({ source: "file", path: "src/guide.md" })).toBe("guide.md")
    expect(formatConcernName({ source: "directory", directory: "docs/guide" })).toBe("docs/guide")
    expect(formatConcernName({ source: "directory", directory: "" })).toBe("Root")
    expect(formatConcernName({ source: "rest" })).toBe("Other changes")
    expect(formatAlsoIn(formatConcernName({ source: "token", token: "proxy" }))).toBe("also in proxy")
    expect(formatHome(formatConcernName({ source: "declaration", name: "open" }))).toBe("Its own row is under open")
    expect(
      formatConcernEvidence([
        { kind: "token", detail: "ij-mcp-proxy", joins: 3 },
        { kind: "uses", detail: "open", joins: 2 },
        { kind: "token", detail: "ij-proxy", joins: 1 },
        { kind: "group", detail: null, joins: 1 },
        { kind: "rest", detail: null, joins: 1 },
      ]),
    ).toBe("Joined by the identifiers ij-mcp-proxy, ij-proxy · the uses of open · one group of files · the file or the subject of a change")
    expect(formatConcernEvidence([])).toBeNull()
  })

  it("titles a concern by what it does to its name, a sentence, and a concern of several changes by its name alone, code", () => {
    const title = (name: Parameters<typeof presentConcernTitle>[0]["name"], change: Parameters<typeof presentConcernTitle>[0]["change"]) => presentConcernTitle({ name, change })
    expect(title({ source: "token", token: "legacy" }, { kind: "removes" })).toEqual({ label: "Remove legacy", sentence: true })
    expect(title({ source: "token", token: "probe" }, { kind: "adds" })).toEqual({ label: "Add probe", sentence: true })
    expect(title({ source: "token", token: "ijproxy" }, { kind: "mixed" })).toEqual({ label: "ijproxy", sentence: false })
    expect(title({ source: "declaration", name: "open" }, { kind: "form", changes: ["parameters"] })).toEqual({
      label: "Change the parameters of open",
      sentence: true,
    })
    expect(title({ source: "declaration", name: "open" }, { kind: "form", changes: ["reach"] }).label).toBe("Change the visibility of open")
    expect(title({ source: "declaration", name: "Store" }, { kind: "form", changes: ["constructors"] }).label).toBe("Change the constructors of Store")
    expect(title({ source: "declaration", name: "open" }, { kind: "form", changes: ["parameters", "reach"] }).label).toBe("Change the signature of open")
    expect(title({ source: "declaration", name: "open" }, { kind: "form", changes: ["header"] }).label).toBe("Change the signature of open")
    expect(title({ source: "declaration", name: "open" }, { kind: "body" }).label).toBe("Change open")
    expect(title({ source: "directory", directory: "rules" }, { kind: "adds" }).label).toBe("Add rules")
    expect(title({ source: "replacement", removed: "grep", added: "accio" }, { kind: "replaces" })).toEqual({ label: "grep → accio", sentence: false })
    expect(title({ source: "file", path: "docs/guide.md" }, { kind: "mixed" })).toEqual({ label: "guide.md", sentence: false })
    expect(title({ source: "directory", directory: "" }, { kind: "adds" })).toEqual({ label: "Root", sentence: false })
    expect(title({ source: "rest" }, { kind: "mixed" })).toEqual({ label: "Other changes", sentence: false })
  })
})
