import { useEffect, useId, useRef, useState } from "react"
import type { FormEvent, ReactNode } from "react"
import { cn } from "cn"
import { CheckIcon, CircleHelpIcon, FolderSearchIcon, GitBranchIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { CommandLine } from "@/components/shared/CommandLine"
import { InlineCode } from "@/components/shared/InlineCode"
import { INVALID_PREFIX_TEXT, isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { hasFolderDialog } from "@/lib/title-bar"
import { consumeOnboardingPrefixFocus, useOnboardingPrefixFocus } from "@/stores/onboarding"
import { browseRepository, useSessionFailure } from "@/stores/repository"
import { setGlobalUserBranchPrefix } from "@/stores/settings/appSettings"

interface WelcomeCardProps {
  repositoryPath: string | undefined
  firstRun: boolean
  needsPrefix: boolean
}

const STEP_LAYOUT = "flex items-start gap-3 p-3"

export function WelcomeCard({ repositoryPath, firstRun, needsPrefix }: WelcomeCardProps) {
  const hasRepository = repositoryPath !== undefined
  const [helpOpen, setHelpOpen] = useState(false)
  const repositoryHelpId = useId()
  return (
    <div>
      <Card data-testid="welcome-card">
        <CardHeader className="border-b">
          <div className="flex items-center gap-2">
            <GitBranchIcon className="size-5 text-primary" />
            {/* the one island title above text-sm (docs/design/ui-theme.md, "Type sizes") */}
            <h3 className="text-base font-semibold">Welcome to Branch Deck</h3>
          </div>
        </CardHeader>

        <CardContent className="flex flex-col gap-4">
          <p className="text-muted-foreground">
            {firstRun
              ? "Work on one branch, get a real Git branch per feature. Two steps to get started:"
              : hasRepository
                ? "You're almost ready: choose your branch prefix to start managing your branches."
                : "Work on one branch, get a real Git branch per feature. Select a repository to start:"}
          </p>

          {/* Safari drops list semantics with list-style: none unless the list has an explicit role. */}
          <ol role="list" aria-label="Getting started" className="flex list-none flex-col gap-3">
            {!hasRepository && hasFolderDialog && (
              <li aria-current="step" data-testid="welcome-step">
                <Button
                  variant="ghost"
                  className={cn(STEP_LAYOUT, "h-auto w-full cursor-pointer justify-start border-0 text-left whitespace-normal has-data-[icon=inline-end]:pr-3")}
                  aria-label="Select a Git Repository…"
                  aria-describedby={repositoryHelpId}
                  onClick={() => void browseRepository()}
                >
                  <StepNumber number={1} active />
                  <span className="flex min-w-0 flex-1 flex-col gap-2">
                    <span className="flex items-center gap-2 font-medium text-primary">
                      Select a Git Repository…
                      <FolderSearchIcon data-icon="inline-end" />
                    </span>
                    <span id={repositoryHelpId} className="text-sm text-muted-foreground">
                      Browse to a local Git repository.
                    </span>
                  </span>
                </Button>
              </li>
            )}

            {!hasRepository && !hasFolderDialog && (
              <Step number={1} title="Select a Git repository" active>
                <p className="text-sm text-muted-foreground">Choose a repository in the repository selector above.</p>
              </Step>
            )}

            {firstRun && hasRepository && (
              <Step number={1} title="Repository selected" complete>
                <p className="text-sm wrap-anywhere text-muted-foreground">{repositoryPath}</p>
              </Step>
            )}

            {firstRun && !needsPrefix && (
              <Step number={2} title="Choose your branch prefix" active={hasRepository}>
                <p className="text-sm text-muted-foreground" role={hasRepository ? "status" : undefined}>
                  {hasRepository ? "Checking branch prefix…" : "After selecting a repository, set a personal prefix if needed."}
                </p>
              </Step>
            )}

            {hasRepository && needsPrefix && (
              <Step number={firstRun ? 2 : 1} title="Choose your branch prefix" active>
                <p className="text-sm text-muted-foreground">
                  {"Your branches are named "}
                  <InlineCode>{"<prefix>/virtual/<name>"}</InlineCode>: a personal prefix, like your username, keeps them apart from the branches of others.
                </p>
                <BranchPrefixForm repositoryPath={repositoryPath} />
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
          </ol>

          <p className="text-sm text-muted-foreground">Once your repository and branch prefix are ready, you can sync and start organizing your commits.</p>
        </CardContent>
      </Card>
      <BranchPrefixHelpDialog open={helpOpen} onOpenChange={setHelpOpen} />
    </div>
  )
}

/**
 * The prefix, pre-filled with the one git suggests (the local part of the user's email, which the refusal of the
 * session without a prefix carries), saved only on Save
 */
function BranchPrefixForm({ repositoryPath }: { repositoryPath: string | undefined }) {
  const field = useRef<HTMLInputElement>(null)
  const focusPrefix = useOnboardingPrefixFocus(repositoryPath)
  useEffect(() => {
    if (!focusPrefix) {
      return
    }
    let frame = 0
    const focus = () => {
      cancelAnimationFrame(frame)
      // The native picker may answer before WebKit has regained focus. Wait for its focus and next paint.
      frame = requestAnimationFrame(() => {
        if (document.hasFocus()) {
          field.current?.focus()
        }
      })
    }
    window.addEventListener("focus", focus)
    // Native focus also retries a refused subscription and remounts this form. Keep the request until the user acts.
    window.addEventListener("pointerdown", consumeOnboardingPrefixFocus, true)
    window.addEventListener("keydown", consumeOnboardingPrefixFocus, true)
    focus()
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener("focus", focus)
      window.removeEventListener("pointerdown", consumeOnboardingPrefixFocus, true)
      window.removeEventListener("keydown", consumeOnboardingPrefixFocus, true)
    }
  }, [focusPrefix])
  const failure = useSessionFailure()
  const suggestion = failure?.details?.type === "prefixSuggestion" ? failure.details.prefix : ""
  // null until the user types: the suggestion of another refusal (a focus asks again) may come after the card
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
          ref={field}
          value={value}
          onChange={event => setDraft(event.target.value)}
          placeholder="your-name"
          aria-label="Branch prefix to save"
          aria-invalid={invalid || undefined}
          className="w-48"
        />
        <Button type="submit" size="sm" disabled={!isValidBranchPrefix(prefix)}>
          Save
        </Button>
      </div>
      {invalid ? (
        <p className="text-xs text-destructive">{INVALID_PREFIX_TEXT}</p>
      ) : (
        <p className="text-xs text-muted-foreground">
          {"Branches will be named "}
          <InlineCode>{`${prefix || "<prefix>"}/virtual/<name>`}</InlineCode>. You can change it later in Settings.
        </p>
      )}
    </form>
  )
}

// Progress colors: active numbers and titles use primary, upcoming steps are muted, and completed titles stay
// neutral beside a success check. The repository action above follows the same rule.
function Step({ number, title, children, active, complete }: { number: number; title: string; children: ReactNode; active?: boolean; complete?: boolean }) {
  return (
    <li className={STEP_LAYOUT} data-testid="welcome-step" aria-current={active ? "step" : undefined}>
      <StepNumber number={number} active={active} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <h4 className={cn("flex items-center gap-2 font-medium", active ? "text-primary" : !complete && "text-muted-foreground")}>
          {title}
          {complete && <CheckIcon className="size-4 text-success" aria-hidden="true" />}
        </h4>
        {complete && <span className="sr-only">Step {number} complete</span>}
        {children}
      </div>
    </li>
  )
}

function StepNumber({ number, active }: { number: number; active?: boolean }) {
  return (
    <span
      className={cn(
        "mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-medium",
        active ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground",
      )}
      data-testid="welcome-step-number"
    >
      {number}
    </span>
  )
}
