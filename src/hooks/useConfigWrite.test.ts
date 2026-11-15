// @spec openspec/specs/repository/settings/spec.md
// A write of the git config of Settings (src/hooks/useConfigWrite.ts): numbered among the writes of the page, its answer
// the settings read again, or none for a write the writer superseded, which is no failure
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { primaryLinkItems, writeRepositoryConfig } from "@/hooks/useConfigWrite"
import type { RepositoryConfig } from "@/lib/bindings"

const { calls, answers, errors, unlinks } = vi.hoisted(() => ({
  unlinks: [] as string[],
  calls: [] as Array<{ order?: { page: string; seq: number } | null }>,
  answers: [] as Array<{ status: "ok"; data: RepositoryConfig | null } | { status: "error"; error: unknown }>,
  errors: [] as string[],
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    setRepositoryConfig: (params: { order?: { page: string; seq: number } | null }) => {
      calls.push(params)
      return Promise.resolve(answers.shift())
    },
    unlinkPrimary: (params: { repositoryPath: string }) => {
      unlinks.push(params.repositoryPath)
      return Promise.resolve({ status: "ok", data: config })
    },
  },
}))
vi.mock("@/lib/errors", () => ({ notifyError: (title: string) => errors.push(title) }))

const config = { seq: 7 } as RepositoryConfig

beforeEach(() => {
  unlinks.length = 0
  calls.length = 0
  answers.length = 0
  errors.length = 0
})

describe("a write of the git config of Settings", () => {
  it("numbers each write after the one before, and answers a superseded one without an error or the settings", async () => {
    const shown: RepositoryConfig[] = []
    answers.push({ status: "ok", data: config }, { status: "ok", data: null })
    const change = { key: "githubRepository", value: "acme/app" } as const
    expect(await writeRepositoryConfig("/repo", change, "local", "Cannot change the GitHub repository of repo", answer => shown.push(answer))).toBe("taken")
    expect(await writeRepositoryConfig("/repo", change, "local", "Cannot change the GitHub repository of repo", answer => shown.push(answer))).toBe("superseded")
    expect(shown).toEqual([config])
    expect(errors).toEqual([])
    const [first, second] = calls.map(call => call.order!)
    expect(second!.page).toBe(first!.page)
    expect(second!.seq).toBeGreaterThan(first!.seq)
  })

  it("tells a failed write by its title and answers its error", async () => {
    answers.push({ status: "error", error: "could not lock config file" })
    const answer = await writeRepositoryConfig("/repo", { key: "pushSizeLimit", value: "1G" }, "local", "Cannot change the push size limit of repo", () => {})
    expect(answer).toEqual({ failed: "could not lock config file" })
    expect(errors).toEqual(["Cannot change the push size limit of repo"])
  })
})

// openspec/specs/branch/declared-needs/spec.md, "Unlink in the companion": the one builder of the link of a companion
describe("primaryLinkItems", () => {
  const ofPrimary = (primaryProblem: string | null) => ({ ...config, primary: "/work/app", primaryProblem }) as RepositoryConfig
  const run = (write: () => Promise<boolean>) => void write()

  it("offers Unlink of a broken link, which unlinks the companion's side", async () => {
    const [unlink] = primaryLinkItems("/work/ci", ofPrimary("/work/app does not list this repository as a companion: add it again there to link it"), run, () => {})
    expect(unlink).toMatchObject({ label: "Unlink", disabled: false })
    unlink?.onSelect({ surface: "unlink-primary" })
    await vi.waitFor(() => expect(unlinks).toEqual(["/work/ci"]))
  })

  it("keeps Unlink of a whole link disabled with why: the primary removes it", () => {
    expect(primaryLinkItems("/work/ci", ofPrimary(null), run, () => {})).toMatchObject([
      { label: "Unlink", disabled: true, description: "app lists this repository as a companion: remove it there" },
    ])
  })

  it("offers nothing to a repository of no primary", () => {
    expect(primaryLinkItems("/work/app", { ...config, primary: null, primaryProblem: null } as RepositoryConfig, run, () => {})).toEqual([])
  })
})
