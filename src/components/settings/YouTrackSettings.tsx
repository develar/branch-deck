import { useId, useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { ExternalLinkIcon, KeyRoundIcon, LogInIcon, LogOutIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { ReadFailure } from "@/components/settings/SettingRow"
import { LoadingLine } from "@/components/shared/LoadingLine"
import type { YouTrackAuthStatus } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { cancelSignIn, openSignInPageAgain, runSignIn, signOut } from "@/lib/sign-in-run"
import { formatTimestamp } from "@/lib/time"
import { formatYouTrackHost, suggestYouTrackUrls, youTrackTokensPage } from "@/lib/youtrack"
import type { SignInPhase } from "@/stores/auth"
import { retryAuthStatus, useAuthReadError, useAuthStatus, useSignInPhase } from "@/stores/auth"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * Settings → YouTrack (spec/repository/youtrack.spec.md): who is signed in to which YouTrack, else the URL to sign in
 * to, offered from the issue links of `.idea/vcs.xml`. Sign In opens the Hub of the YouTrack in the browser; a token
 * created in YouTrack can be pasted instead: both are `sign_in` of YouTrack (`runSignIn`), one at a time. Until the
 * first answer the page says it reads the sign-in; a failed read says why, with Retry, above the last status.
 */
export function YouTrackSettings() {
  const status = useAuthStatus()?.youtrack ?? null
  const readError = useAuthReadError()
  const signIn = useSignInPhase("youTrack")
  const failure = readError === null ? null : <ReadFailure title="Cannot read the YouTrack sign-in" message={readError} testId="youtrack-auth-error" onRetry={retryAuthStatus} />
  if (status === null) {
    return failure ?? <LoadingLine>Reading the YouTrack sign-in…</LoadingLine>
  }
  return (
    <>
      {failure}
      {status.url !== null && status.problem === null && signIn.phase === "idle" ? (
        <SignedIn status={status} url={status.url} />
      ) : (
        // stays mounted while the browser signs in: a cancel keeps the URL typed
        <SignInForm status={status} phase={signIn.phase} />
      )}
    </>
  )
}

function SignedIn({ status, url }: { status: YouTrackAuthStatus; url: string }) {
  const host = formatYouTrackHost(url)
  const user = status.signedInAs
  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="youtrack-auth" data-status="signedIn">
      <div className="flex flex-col gap-0.5">
        <p>
          {status.source === "environment"
            ? `Uses YOUTRACK_TOKEN for ${host}`
            : `Signed in to ${host} as ${user?.fullName ?? "?"}${user !== null && user.login !== user.fullName ? ` (${user.login})` : ""}`}
        </p>
        <p className="text-xs text-muted-foreground" data-testid="youtrack-auth-detail">
          {describeSignIn(status)}
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        {status.source !== "environment" && (
          <Button size="xs" variant="outline" onClick={() => void signOut("youTrack")}>
            <LogOutIcon data-icon="inline-start" />
            Sign Out
          </Button>
        )}
        {status.manageUrl !== null && (
          <Button size="xs" variant="ghost" onClick={() => openPage(status.manageUrl!)}>
            <ExternalLinkIcon data-icon="inline-start" />
            {status.source === "signIn" ? "Manage Sign-In" : "Manage Tokens"}
          </Button>
        )}
      </div>
    </div>
  )
}

/** How the token was given and how long it lasts: "Through the browser since yesterday 3:00 PM. …" */
function describeSignIn(status: YouTrackAuthStatus): string {
  // "Today 2:32 PM", "Yesterday 9:10 AM", "Oct 3, 2026, 2:32 PM" inside a sentence
  const since = status.signedInAt === null ? "" : ` since ${formatTimestamp(status.signedInAt).replace(/^(Today|Yesterday)/, day => day.toLowerCase())}`
  switch (status.source) {
    case "signIn":
      return `Through the browser${since}. It renews itself until YouTrack ends the sign-in, then Branch Deck asks you to sign in again.`
    case "pastedToken":
      return `With a pasted token${since}. It works until you revoke it in YouTrack.`
    default:
      return "Set in the environment of Branch Deck: unset it to sign in here."
  }
}

function SignInForm({ status, phase }: { status: YouTrackAuthStatus; phase: SignInPhase["phase"] }) {
  // the list itself: a selector that builds a new array would render again and again
  const projects = useRecentProjects(items => items)
  const candidates = suggestYouTrackUrls(projects)
  const [url, setUrl] = useState(() => status.url ?? candidates[0]?.url ?? "")
  const [pasting, setPasting] = useState(false)
  const [token, setToken] = useState("")
  const [saving, setSaving] = useState(false)
  const urlId = useId()
  const tokenId = useId()
  const source = candidates.find(candidate => candidate.url === url)
  const noUrl = url.trim() === ""
  // the token is checked by YouTrack before it is kept; the field stays when it was refused
  const save = () => {
    setSaving(true)
    void runSignIn({ provider: "youTrack", url, token }).then(done => {
      setSaving(false)
      if (done !== null) {
        setToken("")
        setPasting(false)
      }
    })
  }
  // asking the YouTrack for its Hub, before the page opens
  const starting = phase === "waiting" && !saving
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="youtrack-auth" data-status={status.problem ?? "signedOut"}>
      {status.problem !== null && (
        <p className="text-warning" data-testid="youtrack-auth-problem">
          {status.problem === "expired" ? "Your YouTrack sign-in expired: sign in again" : "YouTrack rejected the token: sign in again"}
        </p>
      )}
      <div className="flex flex-col gap-1">
        <label htmlFor={urlId} className="text-xs text-muted-foreground">
          YouTrack URL
        </label>
        <Input
          id={urlId}
          value={url}
          placeholder="https://youtrack.example.com"
          spellCheck={false}
          autoComplete="off"
          data-settings-focus="youtrack-url"
          disabled={phase === "browser"}
          onChange={event => setUrl(event.target.value)}
        />
        {(source !== undefined || noUrl) && (
          <p className="text-xs text-muted-foreground" data-testid="youtrack-url-hint">
            {source === undefined ? "The address you open YouTrack at, e.g. https://youtrack.example.com" : `From .idea/vcs.xml of ${source.repository}`}
          </p>
        )}
        {candidates.length > 1 && (
          <div className="flex flex-wrap gap-1" data-testid="youtrack-url-candidates">
            {candidates.map(candidate => (
              <Button key={candidate.url} size="xs" variant="ghost" aria-pressed={candidate.url === url} onClick={() => setUrl(candidate.url)}>
                {formatYouTrackHost(candidate.url)}
              </Button>
            ))}
          </div>
        )}
      </div>
      {phase === "browser" ? (
        <WaitingForBrowser />
      ) : (
        <div className="flex flex-col gap-1">
          <div>
            <Button size="xs" variant="outline" disabled={phase !== "idle" || noUrl} onClick={() => void runSignIn({ provider: "youTrack", url })}>
              {starting ? <Spinner data-icon="inline-start" /> : <LogInIcon data-icon="inline-start" />}
              Sign In to YouTrack
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">Opens YouTrack in the browser. The sign-in renews itself until YouTrack ends it.</p>
        </div>
      )}
      {phase !== "browser" &&
        (pasting ? (
          <div className="flex flex-col gap-3 rounded-lg border border-border p-3" data-testid="youtrack-token-form">
            <div className="flex flex-col gap-1.5">
              <p className="text-xs text-muted-foreground">For a YouTrack that does not sign in apps in the browser: create a token there, then paste it here</p>
              <div>
                <Button size="xs" variant="outline" disabled={noUrl} onClick={() => openPage(youTrackTokensPage(url.trim()))}>
                  <ExternalLinkIcon data-icon="inline-start" />
                  Create Token…
                </Button>
              </div>
            </div>
            <div className="flex flex-col gap-1">
              <label htmlFor={tokenId} className="text-xs text-muted-foreground">
                Token
              </label>
              <Input id={tokenId} type="password" value={token} autoComplete="off" spellCheck={false} placeholder="perm-…" onChange={event => setToken(event.target.value)} />
            </div>
            <div className="flex flex-wrap gap-2">
              <Button size="xs" disabled={saving || token.trim() === "" || noUrl} onClick={save}>
                {saving && <Spinner data-icon="inline-start" />}
                Save Token
              </Button>
              <Button size="xs" variant="ghost" onClick={() => setPasting(false)}>
                Cancel
              </Button>
            </div>
          </div>
        ) : (
          <div>
            {/* a link at the edge of the text: the fallback, below the sign-in */}
            <Button size="xs" variant="link" className="h-auto px-0! text-muted-foreground" onClick={() => setPasting(true)}>
              <KeyRoundIcon data-icon="inline-start" />
              Use a Token Instead
            </Button>
          </div>
        ))}
    </div>
  )
}

function WaitingForBrowser() {
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border p-3 text-sm" data-testid="youtrack-sign-in-waiting">
      <p className="text-muted-foreground">Sign in to YouTrack in the browser tab that opened, then return here</p>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="xs" variant="outline" onClick={() => void openSignInPageAgain("youTrack")}>
          <ExternalLinkIcon data-icon="inline-start" />
          Open the Page Again
        </Button>
        <Button size="xs" variant="ghost" onClick={() => void cancelSignIn("youTrack")}>
          Cancel
        </Button>
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Spinner className="size-3" />
          Waiting for the browser…
        </span>
      </div>
    </div>
  )
}

function openPage(url: string): void {
  void openUrl(url).catch((error: unknown) => notifyError("Cannot open YouTrack", error))
}
