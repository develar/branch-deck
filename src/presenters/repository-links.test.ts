// @spec openspec/specs/branch/declared-needs/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { RepositoryLink } from "@/lib/bindings"
import { projectSections } from "@/presenters/repository-links"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"

function project(path: string): ProjectMetadata {
  return { path }
}

function link(name: string, path: string, role: RepositoryLink["role"] = "companion", problem: string | null = null): RepositoryLink {
  return { name, path, role, problem }
}

describe("project sections", () => {
  it("puts linked repositories after Recent and removes their duplicate recent entries", () => {
    const recentCompanion = project("/work/ci/")
    const sections = projectSections("/work/app/", [link("ci", "/work/ci"), link("docs", "/work/docs")], [project("/work/app"), recentCompanion, project("/work/other")])

    expect(sections.map(section => section.label)).toEqual(["Recent", "Companions of app"])
    expect(sections[0]!.items.map(item => item.project.path)).toEqual(["/work/app", "/work/other"])
    expect(sections[1]!.items.map(item => item.project.path)).toEqual(["/work/ci/", "/work/docs"])
    expect(sections[1]!.items[0]).toMatchObject({ project: recentCompanion, isRecent: true })
  })

  it("names a companion's primary group and keeps the primary first", () => {
    const sections = projectSections("/work/ci", [link("app", "/work/app", "primary"), link("docs", "/work/docs")], [project("/work/ci")])

    expect(sections.map(section => section.label)).toEqual(["Recent", "Primary and companions"])
    expect(sections[1]!.items.map(item => item.project.path)).toEqual(["/work/app", "/work/docs"])
  })

  it("returns one unnamed section when there are no links", () => {
    const recent = [project("/work/app"), project("/work/other")]

    expect(projectSections("/work/app", [], recent)).toEqual([
      {
        label: null,
        items: recent.map(project => ({ project, link: null, isRecent: true })),
      },
    ])
  })

  it("deduplicates equivalent paths and keeps unreadable links in the linked section", () => {
    const unreadable = link("ci", "/work/ci/", "companion", "/work/ci is gone")
    const sections = projectSections("/work/app/", [link("app", "/work/app", "primary"), unreadable, link("ci-copy", "\\work\\ci")], [project("/work/app")])

    expect(sections.map(section => section.label)).toEqual(["Recent", "Companions of app"])
    expect(sections[0]!.items.map(item => item.project.path)).toEqual(["/work/app"])
    expect(sections[1]!.items).toEqual([{ project: { path: "/work/ci/" }, link: unreadable, isRecent: false }])
  })
})
