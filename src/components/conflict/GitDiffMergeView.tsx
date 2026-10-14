import { useMemo } from "react"
import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { DiffModeEnum, DiffView } from "@git-diff-view/react"
import { GitBranchIcon, GitCommitHorizontalIcon, GitPullRequestIcon, InfoIcon } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { useDefaultLayout } from "react-resizable-panels"
import { Alert } from "@/components/ui/alert"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { useIsDark } from "@/hooks/useIsDark"
import type { ConflictDetail, FileDiff, MergeConflictInfo } from "@/lib/bindings"

export interface GitDiffMergeViewProps {
  conflicts: readonly ConflictDetail[]
  conflictInfo?: MergeConflictInfo
}

interface CommitRef {
  hash: string
  subject?: string
  message: string
  author: string
  authorTime: number
}

/** 3-way view of each conflicting file: base (common ancestor), target branch and cherry-picked commit side by side. */
export function GitDiffMergeView({ conflicts, conflictInfo }: GitDiffMergeViewProps) {
  const analysis = conflictInfo?.conflictAnalysis
  const mergeBase: CommitRef | null = analysis?.mergeBaseHash
    ? { hash: analysis.mergeBaseHash, subject: analysis.mergeBaseSubject, message: analysis.mergeBaseMessage, author: analysis.mergeBaseAuthor, authorTime: analysis.mergeBaseTime }
    : null
  const target: CommitRef | null = conflictInfo?.targetBranch ?? null
  const cherry: CommitRef | null = conflictInfo?.commit ?? null

  return (
    <div className="flex flex-col gap-4">
      <Alert className="bg-muted/50 px-4 py-3">
        <InfoIcon />
        <div className="flex flex-col gap-2 text-sm text-foreground">
          <p className="font-medium">Understanding 3-way merge conflicts</p>
          <p>This view shows how both branches evolved from their common ancestor, helping you understand why the conflict occurred:</p>
          <ul className="flex list-inside list-disc flex-col gap-1 text-muted-foreground">
            <li>
              <strong>Base (Common Ancestor)</strong>
              : The original content before the branches diverged
            </li>
            <li>
              <strong>Target Branch</strong>
              : How the file changed in your current branch
            </li>
            <li>
              <strong>Cherry-pick</strong>
              : How the file changed in the commit you&apos;re trying to apply
            </li>
          </ul>
          <p className="text-xs text-muted-foreground">
            The conflict occurs because both branches modified the same parts of the file differently.
          </p>
        </div>
      </Alert>

      {conflicts.length > 0
        ? (
            <AccordionPrimitive.Root multiple className="flex flex-col gap-3">
              {conflicts.map((conflict, index) => (
                <AccordionPrimitive.Item
                  key={`${index}-${conflict.file}`}
                  value={`item-${index}`}
                  className="overflow-hidden rounded-lg border border-border"
                >
                  <CollapsibleFileHeader fileName={conflict.file} />
                  <AccordionPrimitive.Panel className="border-t border-border p-4">
                    <ThreeWayPanels conflict={conflict} index={index} mergeBase={mergeBase} target={target} cherry={cherry} />
                  </AccordionPrimitive.Panel>
                </AccordionPrimitive.Item>
              ))}
            </AccordionPrimitive.Root>
          )
        : (
            <p className="text-sm text-muted-foreground">No conflicts to display</p>
          )}
    </div>
  )
}

interface ThreeWayPanelsProps {
  conflict: ConflictDetail
  index: number
  mergeBase: CommitRef | null
  target: CommitRef | null
  cherry: CommitRef | null
}

// the base panel starts collapsed
const initialLayout = { base: 0, target: 50, cherry: 50 }

/** Mounted only while the accordion item is open */
function ThreeWayPanels({ conflict, index, mergeBase, target, cherry }: ThreeWayPanelsProps) {
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: `conflict-${index}`, storage: localStorage, onlySaveAfterUserInteractions: true })
  const baseDiff = useMemo(() => getBaseDiffData(conflict), [conflict])

  return (
    <ResizablePanelGroup orientation="horizontal" defaultLayout={defaultLayout ?? initialLayout} onLayoutChanged={onLayoutChanged} className="h-[600px] w-full">
      <ResizablePanel id="base" collapsible collapsedSize={0} minSize="15">
        <DiffPanel
          icon={GitCommitHorizontalIcon}
          title="Base (Common Ancestor)"
          description={(
            <>
              {"File state at "}
              {mergeBase ? <CommitPopover commit={mergeBase} /> : <span>merge base</span>}
            </>
          )}
          diff={baseDiff}
        />
      </ResizablePanel>

      <PanelHandle />

      <ResizablePanel id="target" minSize="20">
        <DiffPanel
          icon={GitBranchIcon}
          title="Target Branch (Current)"
          description={(
            <>
              {"Changes from base to current HEAD "}
              {target && <CommitPopover commit={target} />}
            </>
          )}
          diff={conflict.baseToTargetDiff}
        />
      </ResizablePanel>

      <PanelHandle />

      <ResizablePanel id="cherry" minSize="20">
        <DiffPanel
          icon={GitPullRequestIcon}
          title="Cherry-pick (Incoming)"
          description={(
            <>
              {"Changes from base to "}
              {cherry && <CommitPopover commit={cherry} />}
            </>
          )}
          diff={conflict.baseToCherryDiff}
        />
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}

function PanelHandle() {
  return <ResizableHandle className="mx-1 w-1 cursor-col-resize transition-colors hover:bg-primary" />
}

function CommitPopover({ commit }: { commit: CommitRef }) {
  return <CommitHashPopover hash={commit.hash} subject={commit.subject} message={commit.message} author={commit.author} authorTime={commit.authorTime} />
}

interface DiffPanelProps {
  icon: LucideIcon
  title: string
  description: ReactNode
  diff: FileDiff
}

function DiffPanel({ icon: Icon, title, description, diff }: DiffPanelProps) {
  const isDark = useIsDark()
  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex items-center justify-between rounded-lg bg-muted/50 px-3 py-2">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <h4 className="flex items-center gap-2 text-sm font-medium text-foreground">
            <Icon className="size-4 shrink-0" />
            {title}
          </h4>
          <p className="text-xs text-muted-foreground">{description}</p>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-border">
        <DiffView
          data={diff}
          diffViewMode={DiffModeEnum.Unified}
          diffViewWrap
          diffViewHighlight
          diffViewFontSize={12}
          diffViewTheme={isDark ? "dark" : "light"}
        />
      </div>
    </div>
  )
}

/** The base content as a single hunk of context lines (no changes) */
function getBaseDiffData(conflict: ConflictDetail): FileDiff {
  const fileName = conflict.file
  const fileLang = fileName.split(".").pop() || "txt"
  const content = conflict.baseFile?.content || ""

  const hunks: string[] = []
  if (content) {
    const lines = content.split("\n")
    // a space prefix marks a context line
    hunks.push(`--- a/${fileName}\n+++ b/${fileName}\n@@ -1,${lines.length} +1,${lines.length} @@${lines.map(line => `\n ${line}`).join("")}`)
  }

  return {
    oldFile: { fileName, fileLang, content },
    newFile: { fileName, fileLang, content },
    hunks,
  }
}
