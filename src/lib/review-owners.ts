import type { CommitFile } from "@/lib/bindings"
import type { BranchReviewGroup } from "@/types/sub-window"

/** What the files of a review are filtered by */
export type ReviewFilter =
  | { readonly kind: "all" }
  /** A reviewer group the code-owner check asks for */
  | { readonly kind: "group"; readonly name: string }
  /** An owner by the `OWNERSHIP` files */
  | { readonly kind: "owner"; readonly name: string }
  /** The files no `OWNERSHIP` rule owns */
  | { readonly kind: "unowned" }

export interface ReviewFilterChip {
  readonly filter: ReviewFilter
  readonly label: string
  /** The files of the review it keeps; null for a group whose files Patronus doesn't tell */
  readonly count: number | null
  /** The code-owner check asks for the approval of the group */
  readonly needsApproval: boolean
}

function matchesPath(paths: ReadonlySet<string>, file: CommitFile): boolean {
  return paths.has(file.path) || (file.oldPath !== null && paths.has(file.oldPath))
}

function sameName(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase()
}

/** The paths a filter keeps among the files, null for all of them (or a group whose files are unknown) */
export function filterPaths(
  filter: ReviewFilter,
  files: readonly CommitFile[],
  groups: readonly BranchReviewGroup[],
  owners: ReadonlyMap<string, string>,
): ReadonlySet<string> | null {
  switch (filter.kind) {
    case "all":
      return null
    case "group": {
      const group = groups.find(it => it.name === filter.name)
      if (group === undefined || group.branchWide || group.paths.length === 0) {
        return null
      }
      const paths = new Set(group.paths)
      return new Set(files.filter(file => matchesPath(paths, file)).map(file => file.path))
    }
    case "owner":
      return new Set(files.filter(file => owners.get(file.path) === filter.name).map(file => file.path))
    case "unowned":
      return new Set(files.filter(file => !owners.has(file.path)).map(file => file.path))
  }
}

/**
 * The chips of a review: all files, the reviewer groups the code-owner check asks for (their files, all of them for a
 * branch-wide group, unknown when Patronus doesn't tell and no owner has the group's name), then the other owners by
 * the `OWNERSHIP` files, most files first, and the files without an owner. No owner chips without `OWNERSHIP` files.
 * While a filter is `active` the chips show, its own too when the files have none of its own (0), so it can be turned
 * off. Just "All" when nothing filters: one owner (or none) has every file and no group asks for an approval.
 */
export function reviewFilterChips(
  files: readonly CommitFile[],
  groups: readonly BranchReviewGroup[],
  owners: ReadonlyMap<string, string>,
  active: ReviewFilter = { kind: "all" },
): ReviewFilterChip[] {
  const all: ReviewFilterChip = { filter: { kind: "all" }, label: "All", count: files.length, needsApproval: false }
  const chips: ReviewFilterChip[] = [all]
  for (const group of groups) {
    const paths = filterPaths({ kind: "group", name: group.name }, files, groups, owners)
    chips.push({
      filter: { kind: "group", name: group.name },
      label: group.name,
      count: group.branchWide ? files.length : paths === null ? null : paths.size,
      needsApproval: true,
    })
  }
  if (owners.size > 0) {
    const counts = new Map<string, number>()
    let unowned = 0
    for (const file of files) {
      const owner = owners.get(file.path)
      if (owner === undefined) {
        unowned++
      } else {
        counts.set(owner, (counts.get(owner) ?? 0) + 1)
      }
    }
    const others = [...counts].filter(([owner]) => !groups.some(group => sameName(group.name, owner)))
    others.sort(([a, countA], [b, countB]) => countB - countA || a.localeCompare(b))
    for (const [owner, count] of others) {
      chips.push({ filter: { kind: "owner", name: owner }, label: owner, count, needsApproval: false })
    }
    if (unowned > 0) {
      chips.push({ filter: { kind: "unowned" }, label: "No Owner", count: unowned, needsApproval: false })
    }
  }
  if (active.kind !== "all") {
    // the chip of the filter shows while it filters, so it can be turned off
    if (!chips.some(chip => sameFilter(chip.filter, active))) {
      chips.push({ filter: active, label: active.kind === "unowned" ? "No Owner" : (active as { name: string }).name, count: 0, needsApproval: false })
    }
    return chips
  }
  // one chip beside "All" that keeps every file filters nothing
  return chips.length === 2 && !chips[1]!.needsApproval && chips[1]!.count === files.length ? [all] : chips
}

/** The owner of every file, when one owner (by the `OWNERSHIP` files) has them all: "18 files · Air Plugin" */
export function soleOwner(files: readonly CommitFile[], owners: ReadonlyMap<string, string>): string | null {
  const first = files.length === 0 ? undefined : owners.get(files[0]!.path)
  return first !== undefined && files.every(file => owners.get(file.path) === first) ? first : null
}

export function sameFilter(a: ReviewFilter, b: ReviewFilter): boolean {
  return a.kind === b.kind && (a.kind === "all" || a.kind === "unowned" || a.name === (b as { name: string }).name)
}

/** "No files of Air Plugin in this range" */
export function filterEmptyText(filter: ReviewFilter): string {
  switch (filter.kind) {
    case "all":
      return "No changed files"
    case "group":
    case "owner":
      return `No files of ${filter.name} in this range`
    case "unowned":
      return "Every file in this range has an owner"
  }
}
