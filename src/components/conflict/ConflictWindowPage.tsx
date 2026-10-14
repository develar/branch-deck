import type { ReactNode } from "react"
import { Container } from "@/components/layout/Container"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { ConflictCommitContext } from "@/components/conflict/ConflictCommitContext"
import { ConflictResolutionHint } from "@/components/conflict/ConflictResolutionHint"
import { formatConflictTitle } from "@/lib/merge-conflict"

export interface ConflictWindowCommit {
  hash: string
  message: string
  authorTime: number
  committerTime: number
}

export interface ConflictWindowPageProps {
  /** The commit that does not apply; null while the window data loads */
  commit: ConflictWindowCommit | null
  branchName?: string
  /** The branch the commit is copied onto (the baseline or the stack parent) */
  onto?: string
  loadingMessage: string
  /** Between the title and the "How to resolve" hint: the divergence insight */
  insight?: ReactNode
  /** The islands below the commit */
  children?: ReactNode
}

/**
 * The page of a conflict sub-window: the commit that does not apply in a context card (title, meta line, insight and the
 * "How to resolve" hint), then the islands of the window. Each top-level block is a `Card` on the canvas.
 */
export function ConflictWindowPage({ commit, branchName, onto, loadingMessage, insight, children }: ConflictWindowPageProps) {
  return (
    <Container className="flex flex-col gap-6 py-4">
      {commit === null
        ? (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Spinner className="size-4" />
              {loadingMessage}
            </p>
          )
        : (
            <>
              {/* the padding of a Card is not merged with `bd-padding-card`: the body carries it */}
              <Card className="gap-0 py-0">
                <div className="flex flex-col gap-4 bd-padding-card">
                  <ConflictCommitContext
                    title={formatConflictTitle(commit.message, onto)}
                    meta={{
                      hash: commit.hash,
                      authorTime: commit.authorTime,
                      committerTime: commit.committerTime,
                      extra: branchName ? [<span key="branch">{branchName}</span>] : undefined,
                    }}
                  />
                  {insight}
                  <ConflictResolutionHint />
                </div>
              </Card>
              {children}
            </>
          )}
    </Container>
  )
}
