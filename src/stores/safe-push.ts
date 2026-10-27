import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { OperationId, OperationStep, PatronusPushMode, RunningOperation, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { stackReadinessKey } from "@/lib/whole-stack"
import { branchSyncStore, useBranchSyncField } from "@/stores/branch-sync"
import { getSelectedRepositoryPath, useSelectedProjectPath } from "@/stores/repository"

/** The two starts of a robot the app offers: a Safe Push and a Dry Run */
export type SafePushMode = Extract<PatronusPushMode, "rebaseAutosquash" | "dryRun">

/**
 * Where the popover of a start opens: at the Safe Push slot of the branch's row, or at the slot of the header of the
 * stack island whose main line the branch ends (the island's Safe Push, also while the island is collapsed)
 */
export type SafePushPlacement = "row" | "stack-header"

/** The popover of a start that has something to say before it starts (`presentLand` of its plan, `confirm`) */
export interface SafePushAsk {
  readonly branchName: string
  readonly mode: SafePushMode
  readonly placement: SafePushPlacement
  /** The merge profile chosen where the start was asked (`Safe Push With ›`); undefined: the branch's own */
  readonly profileId?: string | null
  /** A Safe Push without review, with a merge profile that skips the code-owner review (the Create Pull Request dialog) */
  readonly bypassReview?: boolean
}

/**
 * A start of a subject as its slot shows it ("Checking…", "Waiting…", "Pushing 45%", "Starting…"): the operation of
 * the session that runs it (`RepositoryView.operations`, a `land` intent of the subject), or the click that waits for
 * its plan; no other start of the subject runs meanwhile
 */
export interface LandRun {
  readonly mode: SafePushMode
  /**
   * `checking`: the plan of a click, then the checks of the operation (the target, the landing gate, the retarget, the
   * cancel of the robot it replaces); `waiting`: another holder of the repository lock; then its push and its robot
   */
  readonly step: "checking" | "waiting" | "pushing" | "starting"
  /** The operation; null while the click waits for its plan */
  readonly op: OperationId | null
  /** The virtual branches its push sends, root first (the refs it reserves); empty without a push */
  readonly pushes: readonly string[]
}

export interface SafePushState {
  readonly ask: SafePushAsk | null
  /**
   * By `askingKey` of the repository and the subject: a click that waits for the plan of its start, which a switch of
   * the repository keeps to its own
   */
  readonly asking: Readonly<Record<string, SafePushMode>>
  /** By operation: how far the push of a start this window runs is, or whom it waits for, as its channel tells it */
  readonly progress: Readonly<Record<OperationId, TransferProgress>>
}

export const safePushStore = createStore<SafePushState>(() => ({ ask: null, asking: {}, progress: {} }))

/** The ask of the popover at the slot of `branchName` in `placement`; null when none is open there */
export function useSafePushAsk(branchName: string, placement: SafePushPlacement): SafePushAsk | null {
  return useStore(safePushStore, state => (state.ask?.branchName === branchName && state.ask.placement === placement ? state.ask : null))
}

function askingKey(repositoryPath: string, key: string): string {
  return `${repositoryPath}\n${key}`
}

/** The subject of a start: the branch, or the key of the whole stack of a root (`stackReadinessKey`) */
function landKeyOf(operation: RunningOperation): string | null {
  const { intent } = operation
  if (intent.kind !== "land") {
    return null
  }
  return intent.subject.kind === "branch" ? intent.subject.name : stackReadinessKey(intent.subject.root)
}

/** The operation of the session that starts the subject `key` (a branch, `stack:<root>`) */
function landOperationIn(operations: Readonly<Record<string, RunningOperation>>, key: string): RunningOperation | undefined {
  return Object.values(operations).find(operation => landKeyOf(operation) === key)
}

/** The step of the slot of an operation's step */
function slotStep(step: OperationStep): LandRun["step"] {
  switch (step.kind) {
    case "waitingForLock":
      return "waiting"
    case "pushing":
      return "pushing"
    case "starting":
    case "writing":
      return "starting"
    default:
      return "checking"
  }
}

/** The virtual branches the operation pushes: the refs it reserves, by their names after `{prefix}/virtual/` */
function pushedBranches(operation: RunningOperation): string[] {
  return operation.resources.flatMap(resource => {
    const at = resource.kind === "ref" ? resource.name.indexOf("/virtual/") : -1
    return resource.kind === "ref" && at !== -1 ? [resource.name.slice(at + "/virtual/".length)] : []
  })
}

/** The start of `key` as the operation of the view and the click have it */
function landRunOf(operation: RunningOperation | undefined, asking: SafePushMode | undefined): LandRun | null {
  if (operation !== undefined && operation.intent.kind === "land") {
    const mode = operation.intent.mode === "dryRun" ? "dryRun" : "rebaseAutosquash"
    return { mode, step: slotStep(operation.step), op: operation.id, pushes: pushedBranches(operation) }
  }
  return asking === undefined ? null : { mode: asking, step: "checking", op: null, pushes: [] }
}

/** The start of the subject `key` that runs in the selected repository */
export function useLandRun(key: string): LandRun | null {
  const operation = useBranchSyncField("operations", operations => landOperationIn(operations, key))
  const path = useSelectedProjectPath() ?? ""
  const asking = useStore(safePushStore, state => state.asking[askingKey(path, key)])
  return landRunOf(operation, asking)
}

/** The start of the subject `key` that runs in the repository, by default the selected one */
export function getLandRun(key: string, repositoryPath: string = getSelectedRepositoryPath()): LandRun | null {
  return landRunOf(landOperationIn(branchSyncStore.getState().operations, key), safePushStore.getState().asking[askingKey(repositoryPath, key)])
}

/** How far the push of the start `op` this window runs is, or whom it waits for; null before its channel told it */
export function useLandProgress(op: OperationId | null): TransferProgress | null {
  return useStore(safePushStore, state => (op === null ? null : (state.progress[op] ?? null)))
}

/** Opens the popover of the start; replaces the popover of another start */
export function openSafePushPopover(ask: SafePushAsk): void {
  safePushStore.setState({ ask })
}

/** Closes the popover, of `branchName` only when given (another may have replaced it meanwhile) */
export function closeSafePushPopover(branchName?: string): void {
  const { ask } = safePushStore.getState()
  if (ask !== null && (branchName === undefined || ask.branchName === branchName)) {
    safePushStore.setState({ ask: null })
  }
}

/** Marks the click on the start of `key` in `repositoryPath` that waits for its plan; null when the wait ended */
export function setLandAsking(key: string, mode: SafePushMode | null, repositoryPath: string): void {
  const asked = askingKey(repositoryPath, key)
  safePushStore.setState(state => {
    const { [asked]: current, ...rest } = state.asking
    if (mode === null) {
      return current === undefined ? state : { asking: rest }
    }
    return current === mode ? state : { asking: { ...rest, [asked]: mode } }
  })
}

/** Records how far the push of the start `op` is, as its channel tells it; null forgets it once the start ended */
export function setLandProgress(op: OperationId, progress: TransferProgress | null): void {
  safePushStore.setState(state => {
    const { [op]: current, ...rest } = state.progress
    if (progress === null) {
      return current === undefined ? state : { progress: rest }
    }
    return { progress: { ...rest, [op]: progress } }
  })
}

/**
 * Stops the start `op` (`cancel_operation`): a wait for another holder of the repository lock ends with nothing pushed
 * or started, and no effect starts after it; a push that sends goes to its end
 */
export async function stopStart(op: OperationId): Promise<void> {
  await commands.cancelOperation({ op })
}

/** Forgets every ask and start (tests) */
export function resetSafePush(): void {
  safePushStore.setState({ ask: null, asking: {}, progress: {} })
}
