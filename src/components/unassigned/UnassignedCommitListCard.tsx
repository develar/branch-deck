import { useCallback, useEffect, useState } from "react"
import type { KeyboardEvent } from "react"
import { GitBranchIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { CommitList } from "@/components/commit/CommitList"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { InlineBranchCreator } from "@/components/unassigned/InlineBranchCreator"
import { UnassignedCommitsHelpPopover } from "@/components/unassigned/UnassignedCommitsHelpPopover"
import { getUnassignedCommitContextMenuItems } from "@/hooks/useUnassignedCommitContextActions"
import { useRowSelection } from "@/hooks/useRowSelection"
import type { Commit } from "@/lib/bindings"
import { getCommitHash } from "@/lib/commit"
import { useIsRowProcessing } from "@/stores/inline-row-action"

const portalId = "inline-branch-creator-portal"

export interface UnassignedCommitListCardProps {
  commits: Commit[]
}

/** Commits that match no grouping pattern; select them to group them into a new branch. */
export function UnassignedCommitListCard({ commits }: UnassignedCommitListCardProps) {
  const [isInlineCreationActive, setIsInlineCreationActive] = useState(false)
  const isCreating = useIsRowProcessing("branch-creation")

  const selection = useRowSelection<Commit>({
    items: commits,
    getId: getCommitHash,
    onSelectionChange: (selectedItems) => {
      if (selectedItems.length === 0) {
        setIsInlineCreationActive(false)
      }
    },
  })
  const { selectedItems, selectedCount, clearSelection } = selection

  const activateInlineCreation = useCallback(() => {
    setIsInlineCreationActive(true)
  }, [])

  // make sure the inline form is visible
  useEffect(() => {
    if (!isInlineCreationActive) {
      return
    }
    const portal = document.getElementById(portalId)
    if (portal === null) {
      return
    }
    const rect = portal.getBoundingClientRect()
    if (rect.top < 0 || rect.bottom > window.innerHeight) {
      portal.scrollIntoView({ behavior: "smooth", block: "center" })
    }
  }, [isInlineCreationActive])

  const handleKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // the open form handles Escape; the selection must not be cleared
    if (isInlineCreationActive && event.key === "Escape") {
      event.preventDefault()
      return
    }
    if (event.key === "Enter" && selectedCount > 0) {
      event.preventDefault()
      activateInlineCreation()
    }
  }

  const handleBranchSuccess = () => {
    setIsInlineCreationActive(false)
    clearSelection()
  }

  // no menu while the form is open
  const contextMenuItems = (selectedCommits: Commit[]) => isInlineCreationActive
    ? []
    : getUnassignedCommitContextMenuItems(selectedCommits, activateInlineCreation)

  return (
    <Card className="gap-0 overflow-hidden py-0" data-testid="unassigned-commits-section">
      <div className="border-b border-border p-4 sm:px-6">
        <SectionCardHeader
          title="Unassigned Commits"
          count={commits.length}
          itemSingular="commit"
          itemPlural="commits"
          badgeColor="neutral"
          actions={commits.length > 0 && (
            <>
              {/* the selection's action lives in the card header, next to what it acts on */}
              {selectedCount > 0 && !isInlineCreationActive && (
                <div className="flex items-center gap-2" data-testid="selection-actions">
                  <span className="text-xs text-muted-foreground">{`${selectedCount} selected`}</span>
                  <Button size="xs" variant="outline" className="border-primary/50 text-primary hover:bg-primary/10 hover:text-primary" onClick={activateInlineCreation}>
                    <GitBranchIcon data-icon="inline-start" />
                    Group into Branch
                  </Button>
                </div>
              )}
              <UnassignedCommitsHelpPopover />
            </>
          )}
        />
      </div>

      <CommitList
        commits={commits}
        variant="compact"
        showAuthor
        editable
        selection={selection}
        contextMenuItems={contextMenuItems}
        onKeyDown={handleKeyDown}
        portalTarget={isInlineCreationActive && (
          <div id={portalId} className="relative overflow-hidden">
            {isCreating && <InlineRowProgressBar />}
            <InlineBranchCreator
              selectedCommits={selectedItems}
              isActive={isInlineCreationActive}
              onCancel={() => setIsInlineCreationActive(false)}
              onSuccess={handleBranchSuccess}
            />
          </div>
        )}
      />
    </Card>
  )
}
