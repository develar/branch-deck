import { ExternalLinkIcon, LogInIcon, LogOutIcon } from "lucide-react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useNow } from "@/hooks/useNow"
import type { GitHubAuthStatus } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatTokenSource } from "@/lib/pull-request"
import { cancelGitHubSignIn, copyCodeAndOpenGitHub, signOutOfGitHub, startGitHubSignIn, useGitHubAuthStatus, useGitHubSignIn } from "@/stores/github-auth"

/**
 * The GitHub part of Settings: where the token comes from, and the app's own sign-in (spec/repository/github-sign-in.spec.md).
 * Signing in shows the code to enter on GitHub until GitHub answers.
 */
export function GitHubSettings() {
  const status = useGitHubAuthStatus()
  const signIn = useGitHubSignIn()
  if (status === null) {
    return null
  }
  if (signIn.phase === "waiting") {
    return <SignInCode userCode={signIn.userCode} expiresAt={signIn.expiresAt} />
  }
  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="github-auth" data-status={status.signedInAs === null ? "signedOut" : "signedIn"}>
      <p>{describeToken(status)}</p>
      <div className="flex flex-wrap gap-2">
        {status.signedInAs === null
          ? status.signInAvailable && (
            <Button size="xs" variant="outline" disabled={signIn.phase === "starting"} onClick={() => void startGitHubSignIn()}>
              {signIn.phase === "starting" ? <Spinner data-icon="inline-start" /> : <LogInIcon data-icon="inline-start" />}
              Sign In to GitHub
            </Button>
          )
          : (
              <Button size="xs" variant="outline" onClick={() => void signOutOfGitHub()}>
                <LogOutIcon data-icon="inline-start" />
                Sign Out
              </Button>
            )}
        {status.manageUrl !== null && status.signedInAs !== null && (
          <Button size="xs" variant="ghost" onClick={() => void openUrl(status.manageUrl!).catch((error: unknown) => notifyError("Cannot open GitHub", error))}>
            <ExternalLinkIcon data-icon="inline-start" />
            Manage on GitHub
          </Button>
        )}
      </div>
    </div>
  )
}

/** "Signed in as @octocat", else which token the app takes, else what gives it one */
function describeToken(status: GitHubAuthStatus): string {
  const source = status.tokenSource
  if (status.signedInAs !== null) {
    return source === null || source === "branchDeck"
      ? `Signed in as @${status.signedInAs}`
      : `Signed in as @${status.signedInAs}; the token of ${formatTokenSource(source)} comes first while GitHub accepts it`
  }
  if (source !== null) {
    return `Uses the token of ${formatTokenSource(source)}`
  }
  return status.signInAvailable
    ? "No GitHub token: sign in, or Branch Deck takes the token of gh, GH_TOKEN or the git credentials of github.com"
    : "No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git"
}

function SignInCode({ userCode, expiresAt }: { userCode: string, expiresAt: number }) {
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border p-3 text-sm" data-testid="github-sign-in-code">
      <p className="text-muted-foreground">Enter this code on GitHub to sign in Branch Deck</p>
      <span className="font-mono text-lg tracking-widest select-all" data-testid="github-user-code">{userCode}</span>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="xs" onClick={() => void copyCodeAndOpenGitHub()}>
          <ExternalLinkIcon data-icon="inline-start" />
          Copy Code and Open GitHub
        </Button>
        <Button size="xs" variant="ghost" onClick={() => void cancelGitHubSignIn()}>Cancel</Button>
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Spinner className="size-3" />
          <Countdown expiresAt={expiresAt} />
        </span>
      </div>
    </div>
  )
}

/** "Waiting for GitHub · expires in 14:32", renewed every second */
function Countdown({ expiresAt }: { expiresAt: number }) {
  const now = useNow(1000)
  const seconds = Math.max(0, Math.round((expiresAt - now) / 1000))
  return `Waiting for GitHub · expires in ${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
}
