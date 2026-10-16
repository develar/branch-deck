import { describe, expect, it } from "vite-plus/test"
import { tildePath } from "@/lib/home-path"

describe("tildePath", () => {
  it("replaces the home directory itself and a path under it", () => {
    expect(tildePath("/Users/ann", "/Users/ann")).toBe("~")
    expect(tildePath("/Users/ann/.claude/skills", "/Users/ann")).toBe("~/.claude/skills")
    expect(tildePath("/Users/ann/.claude/skills", "/Users/ann/")).toBe("~/.claude/skills")
    expect(tildePath("C:\\Users\\ann\\.claude\\skills", "C:\\Users\\ann")).toBe("~\\.claude\\skills")
  })

  it("keeps a sibling of the home directory and any path without a home directory", () => {
    expect(tildePath("/Users/ann2/project", "/Users/ann")).toBe("/Users/ann2/project")
    expect(tildePath("/opt/bd", "/Users/ann")).toBe("/opt/bd")
    expect(tildePath("/Users/ann/project", "")).toBe("/Users/ann/project")
  })
})
