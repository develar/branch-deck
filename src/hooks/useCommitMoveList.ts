import { useCallback, useEffect, useState } from "react"
import type { KeyboardEvent } from "react"
import { CombineIcon, FolderInputIcon, GitBranchIcon, GitBranchPlusIcon, UndoDotIcon, UploadIcon } from "lucide-react"
import { getCommitCopyMenuItems } from "@/hooks/useCommitCopyActions"
import { moveCommits } from "@/hooks/useMoveCommits"
import { useRowSelection } from "@/hooks/useRowSelection"
import type { RowSelection } from "@/hooks/useRowSelection"
import { squashFixupInto } from "@/hooks/useSquashFixups"
import { getFixupTarget } from "@/lib/autosquash"
import type { Commit } from "@/lib/bindings"
import { getCommitHash } from "@/lib/commit"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { hasKeyModifier } from "@/lib/preview-keys"
import { pushUpTo } from "@/lib/push-run"
import { describePushUpTo, pushUpToIn } from "@/presenters/push"
import { branchSyncStore, getAutosquashPlan, getBranch, getIssueGroups, getMoveTargets, pushOrdersIn } from "@/stores/branch-sync"
import { claimSelection, requestCommitForm, requestReveal, startCommitDrag, UNASSIGNED_SOURCE, useCommitFormRequest, useCommitMove, useRevealRequest } from "@/stores/commit-move"
import type { CommitFormRequest } from "@/stores/commit-move"

/** Branches listed in the "Move to" submenu */
const MAX_MENU_TARGETS = 8
/** Characters of the target subject in "Squash into …" */
const MAX_MENU_SUBJECT = 40

export interface CommitMoveListOptions<T extends Commit> {
  commits: readonly T[]
  /** `commits` are listed oldest first (a table with stack lanes); the squash dialog still gets them newest first */
  oldestFirst?: boolean
  /** The branch the commits are in; null for the unassigned commits */
  sourceBranch: string | null
}

/** Selection, the "Move to…" form and the move actions of one commit list; render it with `MovableCommitList`. */
export interface CommitMoveList<T extends Commit> {
  sourceBranch: string | null
  selection: RowSelection<T>
  isTargetOpen: boolean
  /** Opens the "Move to…" form for the selection, or for `commits` (which then become the selection) */
  openTarget: (commits?: readonly T[]) => void
  closeTarget: () => void
  /** Label of the action removing the explicit prefix of the selection, or null if no selected commit has one */
  removePrefixLabel: string | null
  removePrefix: () => void
  /** The commits of the open squash dialog, newest first; null while it is closed */
  squashCommits: readonly T[] | null
  /** Opens the squash dialog for the selection, or for `commits` (which then become the selection); needs two commits */
  openSquash: (commits?: readonly T[]) => void
  closeSquash: () => void
  contextMenuItems: (commits: T[]) => MenuItemGroups
  onKeyDown: (event: KeyboardEvent<HTMLTableElement>) => void
  onDragCommits: (commits: readonly T[]) => void
}

/** Whether `CommitSelectionActions` shows anything: commits are selected and the "Move to…" form is closed */
export function hasSelectionActions<T extends Commit>({ selection, isTargetOpen }: CommitMoveList<T>): boolean {
  return selection.selectedCount > 0 && !isTargetOpen
}

/**
 * What moving the commits back does: a commit with an explicit prefix goes to the issue group of its key
 * (`issueGroups`, issue key -> branch name), to its issue branch (`strippedSubject` starts with the key) or to the
 * unassigned commits; a commit that the issue groups put into a branch goes back to the issue branch of its key. Null
 * if no commit has a way back.
 */
export function getRemovePrefixLabel(commits: readonly Commit[], issueGroups: Readonly<Record<string, string>> = {}): string | null {
  const branches = new Set<string | null>()
  let isGrouped = false
  for (const commit of commits) {
    // the key of the subject itself, as Rust split it: a stripped `(ABC-1) X` names the key of its prefix, which goes
    // with the prefix
    const { prefix, issueKey: key } = commit.subjectParts
    if (prefix !== null) {
      branches.add(key === null ? null : (issueGroups[key] ?? key))
    } else if (key !== null && issueGroups[key] !== undefined) {
      branches.add(key)
      isGrouped = true
    }
  }
  if (branches.size === 0) {
    return null
  }
  if (branches.size > 1) {
    return isGrouped ? "Move Back" : "Remove Prefix"
  }
  const [branch] = branches
  return branch == null ? "Move to Unassigned" : `Move Back to ${branch}`
}

/** "Squash into “Add login”" for a fixup commit with a target */
function getSquashIntoItems(commit: Commit): MenuItem[] {
  const target = getFixupTarget(getAutosquashPlan(), commit.originalHash)
  if (target == null) {
    return []
  }
  const subject = target.targetParts.rest
  const label = subject.length > MAX_MENU_SUBJECT ? `${subject.slice(0, MAX_MENU_SUBJECT - 1)}…` : subject
  return [{ id: "squash-into", label: `Squash into “${label}”`, icon: CombineIcon, onSelect: () => void squashFixupInto(commit.originalHash) }]
}

/**
 * "Push Up to Here" of a commit the remote lacks as the view has it (`pushUpToIn`): the branch up to it, after the
 * branches it is built on that have work, root first; disabled with the reason when it can't; nothing for a commit the
 * remote has, nor for one a merged Safe Push landed, as the row shows no push arrow for either
 */
function getPushUpToItems(branchName: string, commit: Commit): MenuItem[] {
  const state = branchSyncStore.getState()
  const hash = state.branches[branchName]?.commits.find(it => it.originalHash === commit.originalHash)?.hash
  const plan = hash === undefined ? null : pushUpToIn(state, pushOrdersIn(state, branchName), branchName, hash)
  if (plan === null || hash === undefined) {
    return []
  }
  return [
    plan.ok
      ? { id: "push-up-to", label: "Push Up to Here", icon: UploadIcon, description: describePushUpTo(plan.steps, getBranch), onSelect: () => void pushUpTo(branchName, hash) }
      : { id: "push-up-to", label: "Push Up to Here", icon: UploadIcon, disabled: true, description: plan.reason, onSelect: () => {} },
  ]
}

/** How the menu of commits opens the forms of their list */
export interface CommitMenuForms<T extends Commit> {
  readonly openTarget: (commits: readonly T[]) => void
  readonly openSquash: (commits: readonly T[]) => void
  readonly removePrefix: (commits: readonly T[]) => void
}

/**
 * The menu of commits of a list (of `sourceBranch`, null for the unassigned ones), every item by a stable id, one
 * builder for the menu of a row, of its preview and of a commit in the review window: "Move to…", "Move to Existing ›",
 * the way back ("Remove Prefix", "Move Back to …"), the squash, "Push Up to Here", the copies
 */
export function getCommitMenuGroups<T extends Commit>(sourceBranch: string | null, menuCommits: readonly T[], forms: CommitMenuForms<T>): MenuItemGroups {
  if (menuCommits.length === 0) {
    return []
  }
  const isMultiple = menuCommits.length > 1
  const moveItems: MenuItem[] = [
    {
      id: "move-to",
      // the label of the selection button
      label: "Move to…",
      icon: FolderInputIcon,
      kbds: ["m"],
      onSelect: () => forms.openTarget(menuCommits),
    },
  ]
  const targets = getMoveTargets(sourceBranch, MAX_MENU_TARGETS)
  if (targets.length > 0) {
    moveItems.push({
      id: "move-to-existing",
      label: "Move to Existing",
      icon: GitBranchIcon,
      onSelect: () => {},
      children: [
        ...targets.map(name => ({ label: name, icon: GitBranchIcon, onSelect: () => void moveCommits(menuCommits, name) })),
        { label: "New Branch…", icon: GitBranchPlusIcon, onSelect: () => forms.openTarget(menuCommits) },
      ],
    })
  }
  const removeLabel = getRemovePrefixLabel(menuCommits, getIssueGroups())
  if (removeLabel !== null) {
    moveItems.push({ id: "remove-prefix", label: removeLabel, icon: UndoDotIcon, onSelect: () => forms.removePrefix(menuCommits) })
  }
  const squashItems: MenuItem[] = isMultiple
    ? [{ id: "squash", label: `Squash ${menuCommits.length} Commits…`, icon: CombineIcon, kbds: ["s"], onSelect: () => forms.openSquash(menuCommits) }]
    : getSquashIntoItems(menuCommits[0]!)
  const pushItems = isMultiple || sourceBranch === null ? [] : getPushUpToItems(sourceBranch, menuCommits[0]!)
  // a move or a squash during another rewrite goes to the session at once and waits there in the queue
  return [moveItems, squashItems, pushItems, getCommitCopyMenuItems(menuCommits)]
}

export function useCommitMoveList<T extends Commit>({ commits, oldestFirst = false, sourceBranch }: CommitMoveListOptions<T>): CommitMoveList<T> {
  const sourceKey = sourceBranch ?? UNASSIGNED_SOURCE
  const [isTargetOpen, setIsTargetOpen] = useState(false)
  const [squashCommits, setSquashCommits] = useState<readonly T[] | null>(null)

  const selection = useRowSelection<T>({
    items: commits,
    getId: getCommitHash,
    onSelectionChange: selectedItems => {
      if (selectedItems.length === 0) {
        setIsTargetOpen(false)
      } else {
        claimSelection(sourceKey)
      }
    },
  })
  const { selectedItems, selectedCount, clearSelection, selectIds } = selection

  // one selection at a time: another list took it (only a change of the owner counts; this list claims it after
  // its selection changed)
  const selectionOwner = useCommitMove(state => state.selectionOwner)
  useEffect(() => {
    if (selectionOwner !== null && selectionOwner !== sourceKey) {
      clearSelection()
    }
  }, [selectionOwner, sourceKey, clearSelection])

  // commits just moved into this branch are selected
  const reveal = useRevealRequest(sourceKey)
  useEffect(() => {
    if (reveal === null) {
      return
    }
    const present = reveal.commitIds.filter(id => commits.some(commit => getCommitHash(commit) === id))
    if (present.length > 0) {
      selectIds(present)
      claimSelection(sourceKey)
      requestReveal(null)
    }
  }, [reveal, commits, selectIds, sourceKey])

  const openTarget = useCallback(
    (targetCommits?: readonly T[]) => {
      if (targetCommits !== undefined) {
        selectIds(targetCommits.map(getCommitHash))
        claimSelection(sourceKey)
      }
      setIsTargetOpen(true)
    },
    [selectIds, sourceKey],
  )
  const closeTarget = useCallback(() => setIsTargetOpen(false), [])

  const openSquash = (targetCommits?: readonly T[]) => {
    const squashed = targetCommits ?? selectedItems
    if (squashed.length < 2) {
      return
    }
    if (targetCommits !== undefined) {
      selectIds(targetCommits.map(getCommitHash))
      claimSelection(sourceKey)
    }
    setIsTargetOpen(false)
    // the dialog reads them newest first, as git lists them
    setSquashCommits(oldestFirst ? squashed.toReversed() : squashed)
  }
  const closeSquash = useCallback(() => setSquashCommits(null), [])

  const removePrefixFrom = (targetCommits: readonly T[]) => {
    void moveCommits(targetCommits, null).then(success => {
      if (success) {
        clearSelection()
      }
    })
  }

  // no menu while the form is open
  const contextMenuItems = (menuCommits: T[]): MenuItemGroups =>
    isTargetOpen ? [] : getCommitMenuGroups(sourceBranch, menuCommits, { openTarget, openSquash, removePrefix: removePrefixFrom })

  // a form another window asked for (the menu of a commit in the review window), once the list has its commits: the
  // form opens in this render, the selection and the store follow in an effect
  const formRequest = useCommitFormRequest(sourceKey)
  const [openedForm, setOpenedForm] = useState<CommitFormRequest | null>(null)
  if (formRequest !== null && formRequest !== openedForm && formRequest.form !== "reword") {
    const requested = commits.filter(commit => formRequest.commitIds.includes(getCommitHash(commit)))
    if (requested.length > 0) {
      setOpenedForm(formRequest)
      if (formRequest.form === "move") {
        setIsTargetOpen(true)
      } else if (requested.length > 1) {
        setIsTargetOpen(false)
        setSquashCommits(oldestFirst ? requested.toReversed() : requested)
      }
    }
  }
  useEffect(() => {
    if (openedForm === null) {
      return
    }
    selectIds([...openedForm.commitIds])
    claimSelection(sourceKey)
    requestCommitForm(null)
  }, [openedForm, selectIds, sourceKey])

  const onKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // the open form handles Escape; the selection must not be cleared
    if (isTargetOpen && event.key === "Escape") {
      event.preventDefault()
      return
    }
    const isMoveKey = event.key === "Enter" || (event.code === "KeyM" && !event.metaKey && !event.ctrlKey && !event.altKey)
    if (isMoveKey && selectedCount > 0) {
      event.preventDefault()
      setIsTargetOpen(true)
    } else if (event.code === "KeyS" && !hasKeyModifier(event) && selectedCount > 1) {
      event.preventDefault()
      openSquash()
    }
  }

  // the selection when the row is in it, else the row alone (`getActionCommits`): the selection and its form stay, so
  // nothing above the row shows or goes and it doesn't move from under the pointer
  const onDragCommits = (dragged: readonly T[]) => startCommitDrag({ sourceKeys: [sourceKey], branches: [], commits: dragged })

  return {
    sourceBranch,
    selection,
    isTargetOpen,
    openTarget,
    closeTarget,
    removePrefixLabel: getRemovePrefixLabel(selectedItems, getIssueGroups()),
    removePrefix: () => removePrefixFrom(selectedItems),
    squashCommits,
    openSquash,
    closeSquash,
    contextMenuItems,
    onKeyDown,
    onDragCommits,
  }
}
