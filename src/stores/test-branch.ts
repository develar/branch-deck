import type { PushAction, WholeStackView } from "@/lib/bindings"
import type { ParkedState } from "@/lib/parked"
import { layoutStack } from "@/lib/stack-lanes"
import type { BranchStructure, TableRows } from "@/model/structure"
import { NO_ROWS, ROOT_STRUCTURE, rowsOf } from "@/model/structure"
import type { Branch, RemoteStatus, StackBranch, SyncedCommit } from "@/stores/branch-sync"
import { testSubject } from "@/test-support/subject-parts"

/**
 * The actions of a push as Rust decides them of a remote status (`RemoteStatusUpdate::push_action`): a test of the
 * page states the action of the status it sets, as the counts it sets would make Rust answer (the table of
 * `remote_status_test.rs`)
 */
export const CREATE: PushAction = { kind: "create" }
export const PUSH: PushAction = { kind: "push" }
export const UP_TO_DATE: PushAction = { kind: "upToDate" }

/** A force push: `drops` remote commits the branch lacks, `replaces` older versions of its commits */
export function forcePush({ drops = false, replaces = false }: { drops?: boolean; replaces?: boolean } = {}): PushAction {
  return { kind: "force", drops, replaces }
}

/** The remote status of a pushed branch that is up to date; `overrides` replace any field, the action too */
export function testRemoteStatus(overrides: Partial<RemoteStatus> = {}): RemoteStatus {
  return {
    exists: true,
    unpushedCommits: [],
    commitsAhead: 0,
    commitsBehind: 0,
    othersBehind: 0,
    commitsAmended: 0,
    commitsRestacked: 0,
    commitsLanded: 0,
    commitsReworded: 0,
    ancestorCommitsAhead: 0,
    myCommitsAhead: 0,
    fastForward: true,
    lastPushTime: 0,
    remoteTip: null,
    droppedCommits: [],
    action: UP_TO_DATE,
    ...overrides,
  }
}

/** A synced branch of one commit for tests, stacked on `parent` (null: a root); `overrides` replace any field */
/** The synced tip of the branch `name`: `${name}-original`, copied as `${name}-tip`, its subject the name */
export function testTip(name: string): SyncedCommit {
  return {
    originalHash: `${name}-original`,
    hash: `${name}-tip`,
    strippedSubject: name,
    subjectParts: testSubject(name),
    message: name,
    author: "Ann",
    authorTime: 0,
    committerTime: 0,
  }
}

export function testBranch(name: string, parent: string | null, overrides: Partial<Branch> = {}): Branch {
  return {
    name,
    commits: [],
    commitIndex: {},
    commitCount: 1,
    status: "Unchanged",
    statusText: "",
    processedCount: 0,
    hasError: false,
    latestCommitTime: 0,
    summary: "",
    descriptionTitle: null,
    descriptionLead: null,
    nameIssueKey: null,
    remoteStatus: null,
    allCommitsHaveIssueReferences: false,
    myEmail: null,
    origin: "Prefix",
    issueKey: null,
    stack: {
      declaredParent: parent,
      parentSource: parent === null ? null : "Prefix",
      parentState: parent === null ? null : "Active",
      effectiveBase: parent,
      blockedBy: null,
      problems: [],
      prBaseChanged: false,
      childPrecedesParent: false,
    },
    structure: { ...ROOT_STRUCTURE, parent },
    ...overrides,
  }
}

/** The tables of the page tests: the branches, their order, the parked lines and the stack branches */
export interface TestTable {
  readonly branches: Record<string, Branch>
  readonly branchOrder: readonly string[]
  readonly parked?: Readonly<Record<string, string>>
  readonly stackBranches?: Readonly<Record<string, StackBranch>>
}

/**
 * `table` placed as `repository_model::derive` places a table for the view, so a page test reads the structure the page
 * gets: of each branch its children, descendants and main line by its `structure.parent`, its ancestors by
 * `stack.effectiveBase`, parked by the chain it is built on, the whole stack of each root, and the rows of each section
 * as the page splits the order by it (`rowsOf`). A fixture of the page tests only: the rules are Rust's
 * (`domain_policy::stack`, `domain_policy::parked`), and their table tests pin them
 */
export function placed(table: TestTable): { branches: Record<string, Branch>; rows: TableRows } {
  const { branches, branchOrder, parked = {}, stackBranches = {} } = table
  const names = branchOrder.filter(name => branches[name] !== undefined)
  const parentOf = (name: string) => {
    const parent = branches[name]?.structure.parent ?? null
    return parent !== null && parent !== name && branches[parent] !== undefined ? parent : null
  }
  const children = (name: string) => names.filter(child => parentOf(child) === name)
  const descendants = (name: string) => {
    const found: string[] = []
    const seen = new Set([name])
    const pending = [name]
    while (pending.length > 0) {
      for (const child of children(pending.pop()!)) {
        if (!seen.has(child)) {
          seen.add(child)
          found.push(child)
          pending.push(child)
        }
      }
    }
    return found
  }
  const ancestors = (name: string) => {
    const found: string[] = []
    let base = branches[name]?.stack?.effectiveBase ?? null
    while (base !== null && base !== name && !found.includes(base) && branches[base] !== undefined) {
      found.push(base)
      base = branches[base]?.stack?.effectiveBase ?? null
    }
    return found.reverse()
  }
  const parkedState = (name: string): ParkedState | null => {
    const own = parked[name]
    if (own !== undefined) {
      return { note: own, with: null }
    }
    const seen = [name]
    const builtOn = (branch: string) => {
      const stack = branches[branch]?.stack
      return stack == null ? null : (stack.effectiveBase ?? stack.declaredParent)
    }
    let parent = builtOn(name)
    while (parent !== null && !seen.includes(parent)) {
      const note = parked[parent]
      if (note !== undefined) {
        return { note, with: parent }
      }
      seen.push(parent)
      parent = builtOn(parent)
    }
    return null
  }
  // of a root: its stack branch while it forks, else the last branch of a straight chain that is not parked
  const wholeStack = (name: string): WholeStackView | null => {
    if (parentOf(name) !== null) {
      return null
    }
    const stack = stackBranches[name]
    if (stack !== undefined && stack.members.length > 0) {
      const built = stack.state === "Built" && stack.tip !== null
      return {
        kind: "forking",
        notBuilt: built
          ? null
          : { code: "stackNotBuilt", root: name, state: stack.state, blockedBy: stack.blockedBy?.member ?? null, conflict: stack.conflict, error: stack.error },
      }
    }
    const chain = [name]
    for (let next = children(name); next.length === 1 && !chain.includes(next[0]!); next = children(next[0]!)) {
      chain.push(next[0]!)
    }
    if (children(chain.at(-1)!).length > 1) {
      return null
    }
    const leaf = chain.findLast(it => parkedState(it) === null) ?? name
    return leaf === name ? null : { kind: "chain", leaf }
  }
  const placedBranches: Record<string, Branch> = { ...branches }
  for (const name of names) {
    const state = parkedState(name)
    const member = stackBranches[name]?.members.find(it => parkedState(it) !== null)
    const structure: BranchStructure = {
      parent: parentOf(name),
      children: children(name),
      ancestors: ancestors(name),
      descendants: descendants(name),
      mainLine: layoutStack(name, children)
        .filter(row => row.lanes.lane === 0 && row.name !== name)
        .map(row => row.name),
      parked: state,
      stackParked: member === undefined ? null : { member, parked: parkedState(member)! },
      wholeStack: wholeStack(name),
      section: state === null ? "main" : "parked",
    }
    placedBranches[name] = { ...branches[name]!, structure }
  }
  return {
    branches: placedBranches,
    rows: rowsOf(names, placedBranches, NO_ROWS),
  }
}
