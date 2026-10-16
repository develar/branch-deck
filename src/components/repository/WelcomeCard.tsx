import { useState } from "react"
import type { FormEvent, ReactNode } from "react"
import { CircleHelpIcon, GitBranchIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { CommandLine } from "@/components/shared/CommandLine"
import { InlineCode } from "@/components/shared/InlineCode"
import { isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { useRepository } from "@/stores/repository"
import { setGlobalUserBranchPrefix } from "@/stores/settings/appSettings"

interface WelcomeCardProps {
  hasRepository: boolean
  hasBranchPrefix: boolean
}

export function WelcomeCard({ hasRepository, hasBranchPrefix }: WelcomeCardProps) {
  const [helpOpen, setHelpOpen] = useState(false)
  // the steps shown are numbered from 1: with a repository selected, the prefix is the first step
  const prefixStep = hasRepository ? 1 : 2
  return (
    <div>
      <Card data-testid="welcome-card">
        <CardHeader className="border-b">
          <div className="flex items-center gap-2">
            <GitBranchIcon className="size-5 text-primary" />
            {/* the one island title above text-sm (docs/design/ui-theme.md, "Type sizes") */}
            <h3 className="text-base font-semibold">
              Welcome to Branch Deck
            </h3>
          </div>
        </CardHeader>

        <CardContent className="flex flex-col gap-4">
          <p className="text-muted-foreground">
            {hasRepository
              ? "You're almost ready: choose your branch prefix to start managing your branches."
              : "Work on one branch, get a real Git branch per feature. Two steps to start:"}
          </p>

          <div className="flex flex-col gap-3">
            {!hasRepository && (
              <Step number={1} title="Select a Git repository">
                <p className="text-sm text-muted-foreground">
                  Pick it in the repository selector above, or browse for it with the folder button.
                </p>
              </Step>
            )}

            {!hasBranchPrefix && (
              <Step number={prefixStep} title="Choose your branch prefix">
                <p className="text-sm text-muted-foreground">
                  {"Your branches are named "}
                  <InlineCode>{"<prefix>/virtual/<name>"}</InlineCode>
                  : a personal prefix, like your username, keeps them apart from the branches of others.
                </p>
                <BranchPrefixForm />
                <div className="flex flex-col gap-1.5 text-sm text-muted-foreground">
                  <p className="flex flex-wrap items-center gap-x-1">
                    Or set it in Git for all repositories:
                    <Button variant="link" size="xs" onClick={() => setHelpOpen(true)}>
                      <CircleHelpIcon data-icon="inline-start" />
                      View guide
                    </Button>
                  </p>
                  <CommandLine className="max-w-lg">git config --global branchdeck.branchPrefix your-name</CommandLine>
                </div>
              </Step>
            )}
          </div>

          <p className="text-sm text-muted-foreground">
            {hasBranchPrefix
              ? "Once you select a repository, you can sync your branches and start organizing your commits."
              : "Once it is set, you can sync your branches and start organizing your commits."}
          </p>
        </CardContent>
      </Card>
      <BranchPrefixHelpDialog open={helpOpen} onOpenChange={setHelpOpen} />
    </div>
  )
}

/** The prefix, pre-filled with the one git suggests (the local part of the user's email), saved only on Save */
function BranchPrefixForm() {
  const suggestion = useRepository(state => state.gitSetup?.suggested_branch_prefix ?? "")
  // null until the user types: the suggestion may arrive after the card
  const [draft, setDraft] = useState<string | null>(null)
  const value = draft ?? suggestion
  const prefix = normalizeBranchPrefix(value)
  const invalid = prefix !== "" && !isValidBranchPrefix(prefix)

  const save = (event: FormEvent) => {
    event.preventDefault()
    if (isValidBranchPrefix(prefix)) {
      setGlobalUserBranchPrefix(prefix)
    }
  }

  return (
    <form className="flex flex-col gap-1" onSubmit={save} data-testid="welcome-prefix-form">
      <div className="flex items-center gap-2">
        <Input
          value={value}
          onChange={event => setDraft(event.target.value)}
          placeholder="your-name"
          aria-label="Branch prefix to save"
          aria-invalid={invalid || undefined}
          className="w-48"
        />
        <Button type="submit" size="sm" disabled={!isValidBranchPrefix(prefix)}>Save</Button>
      </div>
      {invalid
        ? <p className="text-xs text-destructive">A prefix cannot contain spaces, "..", or any of ~ ^ : ? * [ \</p>
        : (
            <p className="text-xs text-muted-foreground">
              {"Branches will be named "}
              <InlineCode>{`${prefix || "<prefix>"}/virtual/<name>`}</InlineCode>
              . You can change it later in Settings.
            </p>
          )}
    </form>
  )
}

function Step({ number, title, children }: { number: number, title: string, children: ReactNode }) {
  return (
    <div className="flex gap-3" data-testid="welcome-step">
      <div className="mt-0.5 shrink-0">
        <div className="flex size-6 items-center justify-center rounded-full bg-primary/10 text-xs font-medium text-primary" data-testid="welcome-step-number">
          {number}
        </div>
      </div>
      <div className="flex flex-1 flex-col gap-2">
        <h4 className="font-medium">{title}</h4>
        {children}
      </div>
    </div>
  )
}
