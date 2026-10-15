import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { GitBranchIcon, GitCommitHorizontalIcon, GitPullRequestIcon, InfoIcon } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { useDefaultLayout } from "react-resizable-panels"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { CommitHashPopover } from "@/components/commit/CommitHashPopover"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { CollapsibleHint, HintList } from "@/components/conflict/ConflictResolutionHint"
import { LazyFileContentView, LazyFileDiffView } from "@/components/diff/LazyDiffViews"
import type { ConflictDetail, MergeConflictInfo } from "@/lib/bindings"
import { getMergeBase } from "@/lib/merge-conflict"

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
  const base = analysis?.mergeBaseHash ? getMergeBase(analysis) : null
  const mergeBase: CommitRef | null = base && { ...base, authorTime: base.time }
  const target: CommitRef | null = conflictInfo?.targetBranch ?? null
  const cherry: CommitRef | null = conflictInfo?.commit ?? null

  return (
    <div className="flex flex-col gap-4">
      <CollapsibleHint icon={<InfoIcon className="size-4" />} title="How to read this view">
        <p>Each file shows how both sides changed it since their common ancestor:</p>
        <HintList>
          <li>Base: the file at the common ancestor; drag the left divider to show it;</li>
          <li>Target Branch: the changes from the base to the branch the commit is copied onto;</li>
          <li>Cherry-pick: the changes from the base to the commit that does not apply.</li>
        </HintList>
        <p>The commit conflicts where both sides changed the same lines.</p>
      </CollapsibleHint>

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

  return (
    <ResizablePanelGroup orientation="horizontal" defaultLayout={defaultLayout ?? initialLayout} onLayoutChanged={onLayoutChanged} className="h-[min(600px,60vh)] w-full">
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
          content={<LazyFileContentView name={conflict.file} contents={conflict.baseFile?.content ?? ""} />}
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
          content={<LazyFileDiffView diff={conflict.baseToTargetDiff} diffViewMode="unified" withContents />}
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
          content={<LazyFileDiffView diff={conflict.baseToCherryDiff} diffViewMode="unified" withContents />}
        />
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}

function PanelHandle() {
  return <ResizableHandle className="mx-1 w-1 cursor-col-resize transition-colors hover:bg-border" />
}

function CommitPopover({ commit }: { commit: CommitRef }) {
  return <CommitHashPopover hash={commit.hash} subject={commit.subject} message={commit.message} author={commit.author} authorTime={commit.authorTime} />
}

interface DiffPanelProps {
  icon: LucideIcon
  title: string
  description: ReactNode
  /** The file or the diff the panel shows */
  content: ReactNode
}

function DiffPanel({ icon: Icon, title, description, content }: DiffPanelProps) {
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
        {content}
      </div>
    </div>
  )
}
