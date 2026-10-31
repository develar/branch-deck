import { describe, expect, it } from "vite-plus/test"
import { formatAlsoIn, formatConcernEvidence, formatConcernName, formatHome } from "@/presenters/review-concerns"

describe("the words of the concerns", () => {
  it("names a concern by its evidence, and says why its parts are together", () => {
    expect(formatConcernName({ source: "token", token: "proxy" })).toBe("proxy")
    expect(formatConcernName({ source: "replacement", removed: "grep", added: "accio" })).toBe("grep → accio")
    expect(formatConcernName({ source: "declaration", name: "open" })).toBe("open")
    expect(formatConcernName({ source: "file", path: "src/guide.md" })).toBe("guide.md")
    expect(formatConcernName({ source: "directory", directory: "docs/guide" })).toBe("docs/guide")
    expect(formatConcernName({ source: "directory", directory: "" })).toBe("Root")
    expect(formatConcernName({ source: "rest" })).toBe("Other changes")
    expect(formatAlsoIn({ source: "token", token: "proxy" })).toBe("also in proxy")
    expect(formatHome({ source: "declaration", name: "open" })).toBe("Its own row is under open")
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
})
