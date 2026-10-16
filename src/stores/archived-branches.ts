import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import type { BranchIntegrationInfo, Commit, IntegrationConfidence, UpstreamCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getEffectiveBranchPrefix, getSelectedProject, vcsRequestFactory } from "@/stores/repository"

/**
 * Archived branch, filled in progressively: a placeholder first, then integration info, then (lazily) its commits.
 * `unknown` is a placeholder the sync finished without a result for (its detection failed).
 */
export interface ArchivedBranch {
  readonly name: string
  readonly type: "integrated" | "not-integrated" | "placeholder" | "unknown"
  readonly commitCount: number
  readonly confidence: IntegrationConfidence | undefined
  readonly integratedAt: number
  /** The baseline commit that brought an integrated branch in, if found */
  readonly integrationCommit: UpstreamCommit | undefined
  /** For not-integrated branches with partial integration */
  readonly integratedCount: number
  readonly orphanedCount: number
  /** Summary extracted from the first commit for issue-based branches */
  readonly summary: string
  readonly commits: Commit[] | undefined
  readonly isLoadingCommits: boolean
  readonly hasLoadedCommits: boolean
  readonly loadError: string | undefined
}

export interface ArchivedBranchesState {
  /** Sorted: by integration date (newest first), then by archive date from the branch path */
  order: string[]
  branches: Record<string, ArchivedBranch>
  /** The branches a sync of this session archived, marked "New" until `clearNewlyArchived` */
  newlyArchived: ReadonlySet<string>
}

export const archivedBranchesStore = createStore<ArchivedBranchesState>(() => ({
  order: [],
  branches: {},
  newlyArchived: new Set(),
}))

// ---- hooks ----

export function useArchivedBranchOrder(): string[] {
  return useStore(archivedBranchesStore, useShallow(state => state.order))
}

export function useArchivedBranch(name: string): ArchivedBranch | undefined {
  return useStore(archivedBranchesStore, state => state.branches[name])
}

export function useArchivedBranchCount(): number {
  return useStore(archivedBranchesStore, state => state.order.length)
}

export function useIsNewlyArchived(name: string): boolean {
  return useStore(archivedBranchesStore, state => state.newlyArchived.has(name))
}

// ---- updates ----

function createPlaceholder(name: string): ArchivedBranch {
  return {
    name,
    type: "placeholder",
    // unknown until detection completes
    commitCount: 0,
    confidence: undefined,
    integratedAt: 0,
    integrationCommit: undefined,
    integratedCount: 0,
    orphanedCount: 0,
    summary: "",
    commits: undefined,
    isLoadingCommits: false,
    hasLoadedCommits: false,
    loadError: undefined,
  }
}

const archiveDates = new Map<string, string>()

/** Extracts YYYY-MM-DD from paths like "user/archived/2025-01-19/feature-name"; cached, the sort asks for it per comparison */
function extractArchiveDate(branchName: string): string {
  let date = archiveDates.get(branchName)
  if (date === undefined) {
    date = branchName.match(/\/archived\/(\d{4}-\d{2}-\d{2})\//)?.[1] ?? ""
    if (archiveDates.size > 10_000) {
      archiveDates.clear()
    }
    archiveDates.set(branchName, date)
  }
  return date
}

function compareArchivedBranches(a: ArchivedBranch, b: ArchivedBranch): number {
  // primary: integration date (if available), newest first
  if (a.integratedAt > 0 && b.integratedAt > 0) {
    return b.integratedAt - a.integratedAt
  }
  if (a.integratedAt > 0 && b.integratedAt === 0) {
    return -1
  }
  if (a.integratedAt === 0 && b.integratedAt > 0) {
    return 1
  }
  // secondary: archive date from the path, newest first
  return extractArchiveDate(b.name).localeCompare(extractArchiveDate(a.name))
}

/** `insertionOrder` is the order the branches were reported in; sorting is stable relative to it. */
function sortOrder(insertionOrder: string[], branches: Record<string, ArchivedBranch>): string[] {
  return insertionOrder.toSorted((a, b) => compareArchivedBranches(branches[a]!, branches[b]!))
}

let insertionOrder: string[] = []

function setBranches(branches: Record<string, ArchivedBranch>): void {
  archivedBranchesStore.setState({ branches, order: sortOrder(insertionOrder, branches) })
}

function updateArchivedBranch(name: string, updater: (branch: ArchivedBranch) => ArchivedBranch): void {
  const { branches } = archivedBranchesStore.getState()
  const branch = branches[name]
  if (branch === undefined) {
    return
  }
  const next = updater(branch)
  if (next !== branch) {
    setBranches({ ...branches, [name]: next })
  }
}

/**
 * Reconciles with the archived branch names found by sync: keeps existing records (and their loaded commits), adds
 * placeholders, drops missing. `newlyArchived` (the names this sync archived) are marked "New" in addition to the earlier
 * ones that are still archived.
 */
export function updateFromArchivedNames(names: string[], newlyArchived: string[]): void {
  const { branches: existing, newlyArchived: marked } = archivedBranchesStore.getState()
  const branches: Record<string, ArchivedBranch> = {}
  insertionOrder = []
  for (const name of names) {
    if (name in branches) {
      continue
    }
    branches[name] = existing[name] ?? createPlaceholder(name)
    insertionOrder.push(name)
  }
  const nextMarked = new Set([...marked, ...newlyArchived].filter(name => name in branches))
  archivedBranchesStore.setState({ branches, order: sortOrder(insertionOrder, branches), newlyArchived: nextMarked })
}

/**
 * Marks the placeholders a finished sync left without integration info as `unknown`: their detection failed, and
 * without this they would spin until the next sync, with their commits out of reach.
 */
export function settleArchivedPlaceholders(): void {
  const { branches } = archivedBranchesStore.getState()
  let next: Record<string, ArchivedBranch> | null = null
  for (const [name, branch] of Object.entries(branches)) {
    if (branch.type === "placeholder") {
      next ??= { ...branches }
      next[name] = { ...branch, type: "unknown" }
    }
  }
  if (next !== null) {
    setBranches(next)
  }
}

/** Forgets the archived branches of the previous repository when the window opens another one */
export function resetArchivedBranches(): void {
  insertionOrder = []
  archivedBranchesStore.setState({ order: [], branches: {}, newlyArchived: new Set() })
}

/** Drops the "New" mark of the branches archived in this session (once the user has seen what landed) */
export function clearNewlyArchived(): void {
  if (archivedBranchesStore.getState().newlyArchived.size > 0) {
    archivedBranchesStore.setState({ newlyArchived: new Set() })
  }
}

export function updateFromIntegrationInfo(info: BranchIntegrationInfo): void {
  updateFromIntegrationInfos([info])
}

/**
 * Applies the integration info of several branches in one store update, sorted once. The sync reports every archived
 * branch again (most from its cache): a record whose fields stay the same is kept, so its row does not re-render.
 */
export function updateFromIntegrationInfos(infos: readonly BranchIntegrationInfo[]): void {
  const { branches } = archivedBranchesStore.getState()
  let next: Record<string, ArchivedBranch> | null = null
  for (const info of infos) {
    const current = (next ?? branches)[info.name]
    if (current === undefined) {
      insertionOrder = [...insertionOrder, info.name]
    }
    const updated = withIntegrationInfo(current ?? createPlaceholder(info.name), info)
    if (current !== undefined && sameIntegration(current, updated)) {
      continue
    }
    next ??= { ...branches }
    next[info.name] = updated
  }
  if (next !== null) {
    setBranches(next)
  }
}

function withIntegrationInfo(branch: ArchivedBranch, info: BranchIntegrationInfo): ArchivedBranch {
  const base = {
    ...branch,
    summary: info.summary,
    confidence: undefined,
    integrationCommit: undefined,
    integratedCount: 0,
    orphanedCount: 0,
  }
  switch (info.status.kind) {
    case "integrated": {
      const { commitCount, confidence, integratedAt, commit } = info.status
      return { ...base, type: "integrated", commitCount, confidence, integratedAt: integratedAt ?? 0, integrationCommit: commit ?? undefined }
    }
    case "notIntegrated": {
      const { totalCommitCount, integratedCount, orphanedCount, integratedAt } = info.status
      return {
        ...base,
        type: "not-integrated",
        commitCount: totalCommitCount,
        integratedAt: integratedAt ?? 0,
        integratedCount,
        orphanedCount,
      }
    }
    case "partial":
      // represented as not-integrated without detailed counts
      return { ...base, type: "not-integrated", commitCount: 0, integratedAt: 0 }
  }
}

/** The fields the integration info sets are the same; the rest (the loaded commits) comes from the same record */
function sameIntegration(a: ArchivedBranch, b: ArchivedBranch): boolean {
  return a.type === b.type
    && a.summary === b.summary
    && a.commitCount === b.commitCount
    && a.confidence === b.confidence
    && a.integratedAt === b.integratedAt
    && a.integratedCount === b.integratedCount
    && a.orphanedCount === b.orphanedCount
    && a.integrationCommit?.id === b.integrationCommit?.id
    && a.integrationCommit?.author === b.integrationCommit?.author
    && a.integrationCommit?.committer === b.integrationCommit?.committer
    && a.integrationCommit?.time === b.integrationCommit?.time
}

/** Loads the commits of an archived branch once (no-op if loaded or loading). */
export async function loadCommitsForBranch(branchName: string): Promise<void> {
  const branch = archivedBranchesStore.getState().branches[branchName]
  if (branch === undefined || branch.hasLoadedCommits || branch.isLoadingCommits) {
    return
  }

  updateArchivedBranch(branchName, it => ({ ...it, isLoadingCommits: true, loadError: undefined }))
  try {
    const vcsRequest = vcsRequestFactory.createRequest()
    const result = await commands.getArchivedBranchCommits(vcsRequest.repositoryPath, getEffectiveBranchPrefix(), branchName)
    if (result.status === "ok") {
      updateArchivedBranch(branchName, it => ({ ...it, commits: result.data, hasLoadedCommits: true }))
    }
    else {
      updateArchivedBranch(branchName, it => ({ ...it, loadError: result.error.message }))
    }
  }
  catch (error) {
    updateArchivedBranch(branchName, it => ({ ...it, loadError: error instanceof Error ? error.message : String(error) }))
  }
  finally {
    updateArchivedBranch(branchName, it => ({ ...it, isLoadingCommits: false }))
  }
}

/** Deletes an archived branch. Throws on failure; the caller refreshes the list (sync) afterwards. */
export async function deleteArchivedBranch(branchName: string): Promise<void> {
  const result = await commands.deleteArchivedBranch({
    repositoryPath: getSelectedProject()?.path ?? "",
    branchName,
    branchPrefix: getEffectiveBranchPrefix(),
  })
  if (result.status !== "ok") {
    throw new Error(result.error.message)
  }
}
