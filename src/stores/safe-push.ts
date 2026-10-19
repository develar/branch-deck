import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { PatronusPushMode } from "@/lib/bindings"

/** The two starts of a robot the app offers: a Safe Push and a Dry Run */
export type SafePushMode = Extract<PatronusPushMode, "rebaseAutosquash" | "dryRun">

/**
 * Where the popover of a start opens: at the Safe Push slot of the branch's row, or at the slot of the header of the
 * stack island whose main line the branch ends (the island's Safe Push, also while the island is collapsed)
 */
export type SafePushPlacement = "row" | "stack-header"

/** The popover of a start that has something to say before it starts (`getSafePushDecision`, `confirm`) */
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
 * A start running for a branch, after its popover closed or without one: its step shows in the button of the slot
 * ("Checking…", "Pushing 45%", "Starting…"), and no other start of the branch runs meanwhile
 */
export interface SafePushStarting {
  readonly mode: SafePushMode
  /**
   * `checking`: the facts the decision needs (the review, the profiles, the rules); `target`: the target conflict check
   * of the start, "Checking…" too (the fetch of the target and the replay onto its tip); then the push and the start
   */
  readonly step: "checking" | "target" | "pushing" | "starting"
}

export interface SafePushState {
  readonly ask: SafePushAsk | null
  readonly starting: Readonly<Record<string, SafePushStarting>>
}

export const safePushStore = createStore<SafePushState>(() => ({ ask: null, starting: {} }))

/** The ask of the popover at the slot of `branchName` in `placement`; null when none is open there */
export function useSafePushAsk(branchName: string, placement: SafePushPlacement): SafePushAsk | null {
  return useStore(safePushStore, state => (state.ask?.branchName === branchName && state.ask.placement === placement ? state.ask : null))
}

export function useSafePushStarting(branchName: string): SafePushStarting | null {
  return useStore(safePushStore, state => state.starting[branchName] ?? null)
}

export function getSafePushStarting(branchName: string): SafePushStarting | null {
  return safePushStore.getState().starting[branchName] ?? null
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

/** Marks the step of the running start of `branchName`; null when it ended */
export function setSafePushStarting(branchName: string, starting: SafePushStarting | null): void {
  safePushStore.setState(state => {
    const { [branchName]: current, ...rest } = state.starting
    if (starting === null) {
      return current === undefined ? state : { starting: rest }
    }
    return current?.mode === starting.mode && current.step === starting.step ? state : { starting: { ...rest, [branchName]: starting } }
  })
}

/** Forgets every ask and start (tests) */
export function resetSafePush(): void {
  safePushStore.setState({ ask: null, starting: {} })
}
