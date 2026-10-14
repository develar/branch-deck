import { useCallback, useEffect, useState } from "react"
import type { KeyboardEvent } from "react"
import { CombineIcon, FolderInputIcon, GitBranchIcon, GitBranchPlusIcon, UndoDotIcon } from "lucide-react"
import { getCommitCopyMenuItems } from "@/hooks/useCommitCopyActions"
import { moveCommits } from "@/hooks/useMoveCommits"
import { squashFixupInto } from "@/hooks/useSquashFixups"
import { useRowSelection } from "@/hooks/useRowSelection"
import type { RowSelection } from "@/hooks/useRowSelection"
import { getDisplaySubject, getFixupTarget } from "@/lib/autosquash"
import type { Commit } from "@/lib/bindings"
import { findIssueKey, getCommitHash, getSubjectLine, parseSubject } from "@/lib/commit"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { hasKeyModifier } from "@/lib/preview-keys"
import { getAutosquashPlan, getIssueGroups, getMoveTargets } from "@/stores/branch-sync"
import { claimSelection, requestReveal, startCommitDrag, UNASSIGNED_SOURCE, useCommitMove, useRevealRequest } from "@/stores/commit-move"

/** Branches listed in the "Move to" submenu */
const MAX_MENU_TARGETS = 8
/** Characters of the target subject in "Squash into …" */
const MAX_MENU_SUBJECT = 40

export interface CommitMoveListOptions<T extends Commit> {
  commits: readonly T[]
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
  /** The commits of the open squash dialog, in list order; null while it is closed */
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

function hasExplicitPrefix(commit: Commit): boolean {
  return parseSubject(getSubjectLine(commit.message)).prefix !== null
}

/**
 * What removing the prefix does: the commits go back to the issue group of their key (`issueGroups`, issue key ->
 * branch name), to their issue branch (`strippedSubject` starts with the key) or to the unassigned commits. Null if no
 * commit has an explicit prefix.
 */
export function getRemovePrefixLabel(commits: readonly Commit[], issueGroups: Readonly<Record<string, string>> = {}): string | null {
  const prefixed = commits.filter(hasExplicitPrefix)
  if (prefixed.length === 0) {
    return null
  }
  const branches = new Set(prefixed.map((commit) => {
    const key = findIssueKey(commit.strippedSubject)
    return key === null ? null : (issueGroups[key] ?? key)
  }))
  if (branches.size > 1) {
    return "Remove Prefix"
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
  const subject = getDisplaySubject(target.targetSubject)
  const label = subject.length > MAX_MENU_SUBJECT ? `${subject.slice(0, MAX_MENU_SUBJECT - 1)}…` : subject
  return [{ label: `Squash into “${label}”`, icon: CombineIcon, onSelect: () => void squashFixupInto(commit.originalHash) }]
}

export function useCommitMoveList<T extends Commit>({ commits, sourceBranch }: CommitMoveListOptions<T>): CommitMoveList<T> {
  const sourceKey = sourceBranch ?? UNASSIGNED_SOURCE
  const [isTargetOpen, setIsTargetOpen] = useState(false)
  const [squashCommits, setSquashCommits] = useState<readonly T[] | null>(null)

  const selection = useRowSelection<T>({
    items: commits,
    getId: getCommitHash,
    onSelectionChange: (selectedItems) => {
      if (selectedItems.length === 0) {
        setIsTargetOpen(false)
      }
      else {
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

  const openTarget = useCallback((targetCommits?: readonly T[]) => {
    if (targetCommits !== undefined) {
      selectIds(targetCommits.map(getCommitHash))
      claimSelection(sourceKey)
    }
    setIsTargetOpen(true)
  }, [selectIds, sourceKey])
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
    setSquashCommits(squashed)
  }
  const closeSquash = useCallback(() => setSquashCommits(null), [])

  const removePrefixFrom = (targetCommits: readonly T[]) => {
    void moveCommits(targetCommits, null).then((success) => {
      if (success) {
        clearSelection()
      }
    })
  }

  const contextMenuItems = (menuCommits: T[]): MenuItemGroups => {
    // no menu while the form is open
    if (isTargetOpen || menuCommits.length === 0) {
      return []
    }
    const isMultiple = menuCommits.length > 1
    const moveItems: MenuItem[] = [
      {
        label: isMultiple ? `Move ${menuCommits.length} Commits to Branch…` : "Move to Branch…",
        icon: FolderInputIcon,
        kbds: ["m"],
        onSelect: () => openTarget(menuCommits),
      },
    ]
    const targets = getMoveTargets(sourceBranch, MAX_MENU_TARGETS)
    if (targets.length > 0) {
      moveItems.push({
        label: "Move to Existing",
        icon: GitBranchIcon,
        onSelect: () => {},
        children: [
          ...targets.map(name => ({ label: name, icon: GitBranchIcon, onSelect: () => void moveCommits(menuCommits, name) })),
          { label: "New Branch…", icon: GitBranchPlusIcon, onSelect: () => openTarget(menuCommits) },
        ],
      })
    }
    const removeLabel = getRemovePrefixLabel(menuCommits, getIssueGroups())
    if (removeLabel !== null) {
      moveItems.push({ label: removeLabel, icon: UndoDotIcon, onSelect: () => removePrefixFrom(menuCommits) })
    }
    const squashItems: MenuItem[] = isMultiple
      ? [{ label: `Squash ${menuCommits.length} Commits…`, icon: CombineIcon, kbds: ["s"], onSelect: () => openSquash(menuCommits) }]
      : getSquashIntoItems(menuCommits[0]!)
    return [moveItems, squashItems, getCommitCopyMenuItems(menuCommits)]
  }

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
    }
    else if (event.code === "KeyS" && !hasKeyModifier(event) && selectedCount > 1) {
      event.preventDefault()
      openSquash()
    }
  }

  const onDragCommits = (dragged: readonly T[]) => {
    startCommitDrag({ sourceKey, commits: dragged })
    // the form acts on the selection, which the dragged commits may not be
    setIsTargetOpen(false)
  }

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
