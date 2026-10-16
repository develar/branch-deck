import { useRef } from "react"
import type { ReactNode } from "react"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { CommandLine } from "@/components/shared/CommandLine"
import { InlineCode } from "@/components/shared/InlineCode"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"

interface BranchPrefixHelpDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** How to set the branch prefix: the field of Settings, the git commands, the grouping patterns. */
export function BranchPrefixHelpDialog({ open, onOpenChange }: BranchPrefixHelpDialogProps) {
  const bodyRef = useRef<HTMLDivElement>(null)
  return (
    <PreviewDialogFrame data-testid="branch-prefix-help-dialog" width="md" open={open} onOpenChange={onOpenChange} initialFocus={bodyRef}>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Configure the branch prefix</DialogTitle>
        <p className="text-xs text-muted-foreground">
          The branch prefix is the first part of the names of your virtual branches. A personal prefix, like your username, keeps your branches apart in a shared repository.
        </p>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef}>
        <div className="flex flex-col gap-6 text-sm">
          <HelpBlock title="In Branch Deck">
            <p className="text-muted-foreground">
              Type it in Settings… under Branches. It applies to every repository, takes precedence over the Git setting below, and the Agent CLI (bd) uses it too.
            </p>
          </HelpBlock>

          <HelpBlock title="In Git, for all repositories">
            <p className="text-muted-foreground">Run in a terminal, with your name or initials:</p>
            <CommandLine>git config --global branchdeck.branchPrefix your-name</CommandLine>
          </HelpBlock>

          <HelpBlock title="How it works">
            <p className="text-muted-foreground">Branch Deck groups commits based on message patterns:</p>
            <ul className="flex flex-col gap-2">
              <PatternExample label="Explicit prefix:" message="(auth) Add login form" branch="john/virtual/auth" />
              <PatternExample label="Issue key:" message="ABC-123 Fix login bug" branch="john/virtual/ABC-123" />
              <PatternExample label="With subsystem:" message="[threading] ABC-123 Fix login bug" branch="john/virtual/ABC-123" />
            </ul>
            <p className="text-xs text-muted-foreground">
              {"An explicit prefix takes precedence over an issue key. "}
              <InlineCode>virtual</InlineCode>
              {" always follows the branch prefix."}
            </p>
          </HelpBlock>

          <HelpBlock title="In Git, for one repository">
            <p className="text-muted-foreground">Run inside the repository:</p>
            <CommandLine>git config branchdeck.branchPrefix your-name</CommandLine>
          </HelpBlock>
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}

function HelpBlock({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="font-medium">{title}</h3>
      {children}
    </section>
  )
}

function PatternExample({ label, message, branch }: { label: string; message: string; branch: string }) {
  return (
    <li className="flex flex-col gap-1">
      <span className="font-medium">{label}</span>
      <span className="text-muted-foreground">
        <InlineCode>{message}</InlineCode>
        <span className="px-2">→</span>
        <InlineCode>{branch}</InlineCode>
      </span>
    </li>
  )
}
