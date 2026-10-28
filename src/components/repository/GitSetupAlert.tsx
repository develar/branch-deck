import { CircleXIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { CommandLine } from "@/components/shared/CommandLine"
import { resubscribeSession } from "@/model/store"
import { syncBranches, useBranchSync } from "@/stores/branch-sync"
import { useSessionFailure } from "@/stores/repository"

/**
 * What git itself stands in the way of, by the code of what failed (no check of its own): no usable git, missing or
 * too old (`git_not_found`, the failure of the subscription of the window; destructive, the summary as the title, the
 * install hint below), or a sync git cannot write the notes of without the user's name and email (`no_identity`, the
 * failure of the last run; warning, with the two `git config --global` commands). `null` when git is fine.
 */
export function GitSetupAlert() {
  const failure = useSessionFailure()
  const noIdentity = useBranchSync(state => state.syncErrorCode === "no_identity")
  if (failure?.code === "git_not_found") {
    const [summary, ...hintLines] = failure.message.split("\n")
    return (
      <Alert variant="destructive" data-testid="git-setup-alert">
        <CircleXIcon />
        <AlertTitle>{summary}</AlertTitle>
        <AlertDescription className="flex flex-col gap-2">
          {hintLines.map(line => (
            <Hint key={line} line={line} />
          ))}
          <AgainButton label="Check Again" onClick={() => void resubscribeSession()} />
        </AlertDescription>
      </Alert>
    )
  }
  if (noIdentity) {
    return (
      <Alert variant="warning" data-testid="git-setup-alert">
        <TriangleAlertIcon />
        <AlertTitle>Git does not know your name and email</AlertTitle>
        <AlertDescription className="flex flex-col gap-2">
          <div>Branch Deck records its notes and rewritten commits with them. Set them once for all repositories:</div>
          <CommandLine className="max-w-lg">git config --global user.name "Your Name"</CommandLine>
          <CommandLine className="max-w-lg">git config --global user.email you@example.com</CommandLine>
          <AgainButton label="Sync Again" onClick={() => void syncBranches("manual")} />
        </AlertDescription>
      </Alert>
    )
  }
  return null
}

/** "Install or upgrade Git with: brew install git": the command after the last ": " on a line of its own, to copy */
function Hint({ line }: { line: string }) {
  const separator = line.lastIndexOf(": ")
  const command = separator === -1 ? "" : line.slice(separator + 2)
  if (command === "" || command.startsWith("http")) {
    return <div>{line}</div>
  }
  return (
    <>
      <div>{`${line.slice(0, separator)}:`}</div>
      <CommandLine className="max-w-lg">{command}</CommandLine>
    </>
  )
}

function AgainButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <Button variant="outline" size="sm" className="self-start" onClick={onClick}>
      <RefreshCwIcon data-icon="inline-start" />
      {label}
    </Button>
  )
}
