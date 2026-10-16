// @spec spec/repository/update-project.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { UpdateCommit } from "@/lib/bindings"
import { describeUpstreamCommit, groupConflictFiles } from "@/lib/update-conflict"

const NOW = 1_760_400_000
const HOUR = 60 * 60
const DAY = 24 * HOUR

function upstreamCommit(time: number, committerTime: number): UpdateCommit {
  return { id: "c".repeat(40), subject: "Change line 2 upstream", author: "Dana Lee", time, committer: "Patronus", committerTime, files: [] }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(NOW * 1000)
})

afterEach(() => {
  vi.useRealTimers()
})

describe("describeUpstreamCommit", () => {
  it("names when the commit was written and when it landed, if that was more than an hour later", () => {
    expect(describeUpstreamCommit(upstreamCommit(NOW - 13 * DAY, NOW - 2 * HOUR))).toBe("Dana Lee · authored 13 days ago · landed 2 hours ago")
  })

  it("gives one time when the commit landed within an hour of being written", () => {
    expect(describeUpstreamCommit(upstreamCommit(NOW - 13 * DAY, NOW - 13 * DAY + HOUR))).toBe("Dana Lee, 13 days ago")
    expect(describeUpstreamCommit(upstreamCommit(NOW - 3 * HOUR, NOW - 3 * HOUR))).toBe("Dana Lee, 3 hours ago")
  })
})

describe("groupConflictFiles", () => {
  const commit = (id: string, files: string[]): UpdateCommit => ({ id, subject: id, author: "Me", time: NOW, committer: "Me", committerTime: NOW, files })

  it("gives each file the commits of each side that change it, in their order", () => {
    const local = [commit("a", ["x.txt"]), commit("b", []), commit("c", ["y.txt", "x.txt"])]
    const upstream = [commit("u", ["y.txt"])]
    const ids = (commits: readonly UpdateCommit[]) => commits.map(it => it.id)
    expect(groupConflictFiles(["y.txt", "x.txt", "z.txt"], local, upstream).map(file => [file.path, ids(file.local), ids(file.upstream)])).toEqual([
      ["y.txt", ["c"], ["u"]],
      ["x.txt", ["a", "c"], []],
      ["z.txt", [], []],
    ])
  })
})
