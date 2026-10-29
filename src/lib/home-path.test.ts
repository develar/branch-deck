import { describe, expect, it } from "vite-plus/test"
import { pathForms, tildePath } from "@/lib/home-path"

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

describe("pathForms", () => {
  it("cuts a path only at its separators, dropping whole leading segments behind one …, the name alone last", () => {
    expect(pathForms("~/notes.local/scratch/demo")).toEqual(["~/notes.local/scratch/demo", "…/scratch/demo", "…/demo", "demo"])
    expect(pathForms("/var/folders/T/repo")).toEqual(["/var/folders/T/repo", "…/folders/T/repo", "…/T/repo", "…/repo", "repo"])
    expect(pathForms("C:\\Users\\ann\\demo")).toEqual(["C:\\Users\\ann\\demo", "…\\Users\\ann\\demo", "…\\ann\\demo", "…\\demo", "demo"])
  })

  it("keeps a path without a segment to drop whole, and drops no trailing separator into a form", () => {
    expect(pathForms("~/demo")).toEqual(["~/demo", "demo"])
    expect(pathForms("demo")).toEqual(["demo"])
    expect(pathForms("~")).toEqual(["~"])
    expect(pathForms("/srv//demo/")).toEqual(["/srv//demo/", "…/demo", "demo"])
  })
})
