import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, RefObject } from "react"
import { ExternalLinkIcon, PlusIcon, SearchIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Combobox,
  ComboboxChip,
  ComboboxChips,
  ComboboxChipsInput,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxItem,
  ComboboxList,
  ComboboxValue,
  useComboboxAnchor,
} from "@/components/ui/combobox"
import { DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { useAvailabilityAction } from "@/hooks/useAvailabilityAction"
import { usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { GitHubUser, NewPullRequest } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { createLatestOnly, formatCreatedToast, isLogin, preselectedReviewers, searchResults, suggestedReviewers } from "@/lib/create-pull-request"
import type { SuggestedReviewer } from "@/lib/create-pull-request"
import { notifyError } from "@/lib/errors"
import { getNewPullRequestUrl, getPullRequestBase, openNewPullRequest } from "@/lib/git/new-pull-request"
import { isMac } from "@/lib/platform"
import { formatAvailabilityHint, openPullRequest } from "@/lib/pull-request"
import { showToast } from "@/lib/toast-message"
import { recordPullRequestCreated } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync } from "@/stores/branch-sync"
import { closeCreatePullRequestDialog, useCreatePullRequestBranch, useCreatePullRequestGeneration } from "@/stores/create-pull-request-dialog"
import { usePatronusAvailable } from "@/stores/patronus"
import { createPullRequest, usePullRequest, usePullRequestAvailability } from "@/stores/pull-requests"
import { getFullBranchName, useSelectedProjectPath } from "@/stores/repository"
import { setCreatePullRequestAssignSelf, useAppSettings } from "@/stores/settings/appSettings"

/** A search waits for the typing to pause */
const SEARCH_DEBOUNCE_MS = 250
/** As `MIN_SEARCH_LENGTH` of the backend: a shorter query is not sent */
const MIN_SEARCH_LENGTH = 2

/**
 * The "Create pull request" dialog of the branch the store names: the title and the description (the `bd describe`
 * text, else the pushed commits), the reviewers (the code owners Patronus suggests, one per group chosen at once,
 * and a search of the repository's users), "Assign to me"; ⌘↵ creates it through the GitHub API, "Continue on GitHub"
 * opens GitHub's form with the text instead (it takes no reviewers). Without a token GitHub accepts, it says why and
 * GitHub's form is the way.
 *
 * Loaded on the first open, then kept mounted (see `PreviewDialogFrame`); each opening starts with a new text.
 */
export function CreatePullRequestDialog() {
  const branchName = useCreatePullRequestBranch()
  const generation = useCreatePullRequestGeneration()
  // the last branch stays while the closing animation runs
  const [shown, setShown] = useState<{ branchName: string; generation: number } | null>(null)
  if (branchName !== null && shown?.generation !== generation) {
    setShown({ branchName, generation })
  }
  const [isCreating, setIsCreating] = useState(false)
  const titleRef = useRef<HTMLInputElement>(null)
  const branch = useBranchSync(state => (shown === null ? undefined : state.branches[shown.branchName]))

  return (
    <PreviewDialogFrame
      data-testid="create-pull-request-dialog"
      width="md"
      open={branchName !== null}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        // a click outside keeps the text; nothing closes the dialog while GitHub creates the pull request
        if (eventDetails.reason === "outside-press" || isCreating) {
          eventDetails.cancel()
          return
        }
        closeCreatePullRequestDialog()
      }}
      initialFocus={titleRef}
    >
      {shown !== null && branch !== undefined && (
        <CreatePullRequestForm key={shown.generation} branch={branch} titleRef={titleRef} isCreating={isCreating} setIsCreating={setIsCreating} />
      )}
    </PreviewDialogFrame>
  )
}

interface CreatePullRequestFormProps {
  branch: Branch
  titleRef: RefObject<HTMLInputElement | null>
  isCreating: boolean
  setIsCreating: (isCreating: boolean) => void
}

type TextState = { status: "loading" } | { status: "loaded"; text: NewPullRequest } | { status: "error"; message: string }

function CreatePullRequestForm({ branch, titleRef, isCreating, setIsCreating }: CreatePullRequestFormProps) {
  const repositoryPath = useSelectedProjectPath() ?? ""
  const head = getFullBranchName(branch.name)
  const base = getPullRequestBase(branch)
  const parent = branch.stack?.effectiveBase ?? null
  // the parent's virtual name, never its ref
  const baseName = parent !== null && base === getFullBranchName(parent) ? parent : base

  const [text, setText] = useState<TextState>({ status: "loading" })
  const [title, setTitle] = useState("")
  const [body, setBody] = useState("")
  // the text of the branch fills only what the user hasn't typed yet
  const [edited, setEdited] = useState({ title: false, body: false })
  useEffect(() => {
    let current = true
    commands.getNewPullRequest({ repositoryPath, head, base }).then(
      result => {
        if (!current) {
          return
        }
        setText(result.status === "ok" ? { status: "loaded", text: result.data } : { status: "error", message: result.error.message })
      },
      (error: unknown) => {
        if (current) {
          setText({ status: "error", message: error instanceof Error ? error.message : String(error) })
        }
      },
    )
    return () => {
      current = false
    }
  }, [repositoryPath, head, base])
  const [filled, setFilled] = useState(false)
  if (!filled && text.status === "loaded") {
    setFilled(true)
    if (!edited.title) {
      setTitle(text.text.title)
    }
    if (!edited.body) {
      setBody(text.text.body)
    }
  }
  const viewerLogin = text.status === "loaded" ? text.text.viewerLogin : null

  const availability = usePullRequestAvailability()
  // never asked yet is no reason to refuse: GitHub answers the creation
  const unavailable = availability !== null && availability.status !== "available"
  const availabilityHint = unavailable ? formatAvailabilityHint(availability) : null
  const availabilityAction = useAvailabilityAction(unavailable ? availability : null)

  const patronusAvailable = usePatronusAvailable()
  // the code owners of the changes, whatever a pull request would count
  const review = usePatronusReviewRequirement(branch.name, patronusAvailable, null)
  const suggestions = review.state.status === "loaded" ? suggestedReviewers(review.state.requirement, viewerLogin) : []

  const [reviewers, setReviewers] = useState<GitHubUser[]>([])
  const [preselected, setPreselected] = useState(false)
  // once the code owners and the user are known: the user is never asked to review the own pull request
  if (!preselected && review.state.status !== "loading" && text.status !== "loading") {
    setPreselected(true)
    const chosen = preselectedReviewers(suggestions).filter(user => !reviewers.some(it => isLogin(it.login, user.login)))
    if (chosen.length > 0) {
      setReviewers([...reviewers, ...chosen])
    }
  }

  const assignSelf = useAppSettings(state => state.createPullRequestAssignSelf)

  // a pull request that appeared meanwhile (created elsewhere, or by the refresh after the push)
  const existing = usePullRequest(branch.name)
  const liveExisting = existing !== undefined && existing.state !== "closed" ? existing : null

  const canCreate = !isCreating && !unavailable && liveExisting === null && title.trim() !== "" && !title.includes("\n")

  const create = async () => {
    if (!canCreate) {
      return
    }
    setIsCreating(true)
    const request = { branchName: branch.name, head, base, title: title.trim(), body, reviewers: reviewers.map(user => user.login), assignSelf }
    // `.finally`, not try/finally: React Compiler cannot compile a try without a catch
    const result = await createPullRequest(request)
      .catch((error: unknown) => {
        notifyError("Cannot create pull request", error)
        return null
      })
      .finally(() => setIsCreating(false))
    if (result === null) {
      return
    }
    const toast = formatCreatedToast(branch.name, baseName, viewerLogin, result)
    const pullRequest = result.pullRequest
    if (result.created) {
      recordPullRequestCreated(toast.title, toast.description ?? "", pullRequest.url, toast.type === "warning")
    }
    showToast({ ...toast, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
    closeCreatePullRequestDialog()
  }

  const continueOnGitHub = () => {
    const url = getNewPullRequestUrl(branch, { title: title.trim(), body, assignee: assignSelf ? (viewerLogin ?? undefined) : undefined })
    if (url !== null) {
      openNewPullRequest(url)
      closeCreatePullRequestDialog()
    }
  }

  const search = useReviewerSearch(repositoryPath)
  const choose = (suggestion: SuggestedReviewer) => {
    if (suggestion.login === null) {
      // Patronus doesn't say the login: the user picks the person from what GitHub knows
      search.lookUp(suggestion.fullName)
      return
    }
    if (!reviewers.some(user => isLogin(user.login, suggestion.login))) {
      setReviewers([...reviewers, { login: suggestion.login, name: suggestion.fullName }])
    }
  }
  const unchosen = suggestions.filter(suggestion => suggestion.login === null || !reviewers.some(user => isLogin(user.login, suggestion.login)))

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      void create()
    }
  }

  return (
    <>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Create pull request</DialogTitle>
        <p data-testid="create-pull-request-base" className="truncate text-xs text-muted-foreground">
          {`${branch.name} → ${baseName}`}
        </p>
      </PreviewDialogHeader>

      <PreviewDialogBody>
        {/* ⌘↵ creates from any field, the reviewer search too (its Enter picks a user) */}
        <div className="flex flex-col gap-4" onKeyDown={handleKeyDown}>
          {availabilityHint !== null && (
            <Alert variant="warning" data-testid="create-pull-request-unavailable">
              <TriangleAlertIcon />
              <AlertDescription className="flex flex-wrap items-center gap-x-2">
                {`${availabilityHint}: GitHub's form opens the pull request instead.`}
                {availabilityAction !== null && (
                  <Button variant="link" size="xs" className="h-auto p-0" onClick={availabilityAction.onClick}>
                    {availabilityAction.label}
                  </Button>
                )}
              </AlertDescription>
            </Alert>
          )}
          {liveExisting !== null && (
            <Alert variant="info" data-testid="create-pull-request-exists">
              <AlertDescription className="flex flex-wrap items-center gap-x-2">
                {`${branch.name} already has pull request #${liveExisting.number}.`}
                <Button variant="link" size="xs" className="h-auto p-0" onClick={() => openPullRequest(liveExisting)}>
                  Open Pull Request
                </Button>
              </AlertDescription>
            </Alert>
          )}
          {text.status === "error" && <p className="text-xs text-destructive">{`Cannot read the commits of ${branch.name}: ${text.message}`}</p>}

          <label className="flex flex-col gap-1.5">
            <span className="text-xs text-muted-foreground">Title</span>
            <Input
              ref={titleRef}
              data-testid="create-pull-request-title"
              value={title}
              placeholder={text.status === "loading" ? "Reading the commits…" : undefined}
              disabled={isCreating}
              onChange={event => {
                setEdited({ ...edited, title: true })
                setTitle(event.target.value)
              }}
            />
          </label>
          <label className="flex flex-col gap-1.5">
            <span className="text-xs text-muted-foreground">Description</span>
            <Textarea
              data-testid="create-pull-request-body"
              className="min-h-28"
              value={body}
              disabled={isCreating}
              spellCheck
              onChange={event => {
                setEdited({ ...edited, body: true })
                setBody(event.target.value)
              }}
            />
          </label>
          {text.status === "loaded" && text.text.unpushed > 0 && (
            <p data-testid="create-pull-request-unpushed" className="text-xs text-muted-foreground">
              {text.text.unpushed === 1
                ? `1 commit of ${branch.name} isn't on the remote: the pull request shows the pushed ones`
                : `${text.text.unpushed} commits of ${branch.name} aren't on the remote: the pull request shows the pushed ones`}
            </p>
          )}

          {/* GitHub's form takes no reviewers, and without a token nobody can be searched */}
          {!unavailable && (
            <div className="flex flex-col gap-1.5">
              <span className="text-xs text-muted-foreground">Reviewers</span>
              {patronusAvailable && <CodeOwnerSuggestions state={review.state} suggestions={unchosen} onChoose={choose} />}
              <ReviewerPicker reviewers={reviewers} setReviewers={setReviewers} search={search} viewerLogin={viewerLogin} disabled={isCreating} />
            </div>
          )}

          {/* GitHub's form assigns a login: without a token the app knows none */}
          {(!unavailable || viewerLogin !== null) && (
            <label className="flex items-center gap-2">
              <Switch data-testid="create-pull-request-assign" checked={assignSelf} disabled={isCreating} onCheckedChange={setCreatePullRequestAssignSelf} />
              <span>Assign to me</span>
              {viewerLogin !== null && <span className="text-xs text-muted-foreground">{viewerLogin}</span>}
            </label>
          )}
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
        <Tooltip>
          <TooltipTrigger
            render={<Button data-testid="create-pull-request-on-github" variant={unavailable ? "default" : "ghost"} size="xs" disabled={isCreating} onClick={continueOnGitHub} />}
          >
            <ExternalLinkIcon data-icon="inline-start" />
            Continue on GitHub
          </TooltipTrigger>
          <TooltipContent>GitHub's form takes the title, the description and the assignee, not the reviewers</TooltipContent>
        </Tooltip>
        {!unavailable && (
          <Button data-testid="create-pull-request-submit" size="xs" disabled={!canCreate} onClick={() => void create()}>
            {isCreating && <Spinner data-icon="inline-start" />}
            Create Pull Request
            <Kbd>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
          </Button>
        )}
      </PreviewDialogFooter>
    </>
  )
}

/** "Suggested by code owners": a chip per suggested reviewer of each group whose approval is missing */
function CodeOwnerSuggestions({
  state,
  suggestions,
  onChoose,
}: {
  state: ReturnType<typeof usePatronusReviewRequirement>["state"]
  suggestions: readonly SuggestedReviewer[]
  onChoose: (suggestion: SuggestedReviewer) => void
}) {
  if (state.status === "loading") {
    return (
      <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Spinner className="size-3" />
        Asking Patronus for the code owners…
      </span>
    )
  }
  if (state.status === "error") {
    return <span className="text-xs text-muted-foreground">{`Cannot read the code owners: ${state.message}`}</span>
  }
  if (state.requirement.skipped) {
    return <span className="text-xs text-muted-foreground">Too many commits for Patronus to name the code owners</span>
  }
  if (suggestions.length === 0) {
    return null
  }
  return (
    <div data-testid="create-pull-request-suggestions" className="flex flex-wrap items-center gap-1.5">
      <span className="text-xs text-muted-foreground">Suggested by code owners:</span>
      {suggestions.map(suggestion => (
        <Tooltip key={`${suggestion.group}\n${suggestion.fullName}`}>
          <TooltipTrigger render={<Button variant="outline" size="xs" onClick={() => onChoose(suggestion)} />}>
            {suggestion.login === null ? <SearchIcon data-icon="inline-start" /> : <PlusIcon data-icon="inline-start" />}
            {suggestion.login === null ? suggestion.fullName : `${suggestion.fullName} (${suggestion.login})`}
          </TooltipTrigger>
          <TooltipContent>
            {suggestion.login === null ? `${suggestion.group}: Patronus doesn't say the GitHub login, find ${suggestion.fullName}` : suggestion.group}
          </TooltipContent>
        </Tooltip>
      ))}
    </div>
  )
}

interface ReviewerSearch {
  query: string
  setQuery: (query: string) => void
  results: readonly GitHubUser[]
  isSearching: boolean
  error: string | null
  open: boolean
  setOpen: (open: boolean) => void
  /** Searches `name` and shows what GitHub found */
  lookUp: (name: string) => void
}

/** The search of the users a review can be asked from: debounced, and an older answer never replaces a newer one */
function useReviewerSearch(repositoryPath: string): ReviewerSearch {
  const [query, setQueryState] = useState("")
  const [results, setResults] = useState<readonly GitHubUser[]>([])
  const [isSearching, setIsSearching] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState(false)
  const [latest] = useState(createLatestOnly)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  useEffect(() => () => clearTimeout(timer.current), [])

  const setQuery = (next: string) => {
    setQueryState(next)
    clearTimeout(timer.current)
    const id = latest.next()
    if (next.trim().length < MIN_SEARCH_LENGTH) {
      setResults([])
      setIsSearching(false)
      setError(null)
      return
    }
    setIsSearching(true)
    timer.current = setTimeout(() => {
      commands.searchGithubUsers({ repositoryPath, query: next }).then(
        result => {
          if (!latest.isLatest(id)) {
            return
          }
          setIsSearching(false)
          if (result.status === "ok") {
            setResults(result.data)
            setError(null)
          } else {
            setError(result.error.message)
          }
        },
        (failure: unknown) => {
          if (latest.isLatest(id)) {
            setIsSearching(false)
            setError(failure instanceof Error ? failure.message : String(failure))
          }
        },
      )
    }, SEARCH_DEBOUNCE_MS)
  }

  return {
    query,
    setQuery,
    results,
    isSearching,
    error,
    open,
    setOpen,
    lookUp: name => {
      setQuery(name)
      setOpen(true)
    },
  }
}

/** The chosen reviewers as chips, and the search of more */
function ReviewerPicker({
  reviewers,
  setReviewers,
  search,
  viewerLogin,
  disabled,
}: {
  reviewers: GitHubUser[]
  setReviewers: (reviewers: GitHubUser[]) => void
  search: ReviewerSearch
  viewerLogin: string | null
  disabled: boolean
}) {
  const anchor = useComboboxAnchor()
  const items = searchResults(search.results, reviewers, viewerLogin)
  const empty =
    search.error ?? (search.isSearching ? "Searching…" : search.query.trim().length < MIN_SEARCH_LENGTH ? "Type a login or a name" : "No user of the repository matches")
  return (
    <Combobox
      multiple
      items={items}
      value={reviewers}
      onValueChange={value => {
        setReviewers(value)
        search.setQuery("")
      }}
      inputValue={search.query}
      onInputValueChange={search.setQuery}
      open={search.open}
      onOpenChange={search.setOpen}
      // GitHub filters: the items are its answer
      filter={null}
      itemToStringLabel={(user: GitHubUser) => user.login}
      isItemEqualToValue={(item: GitHubUser, value: GitHubUser) => isLogin(item.login, value.login)}
      disabled={disabled}
    >
      <ComboboxChips ref={anchor} data-testid="create-pull-request-reviewers">
        <ComboboxValue>
          {(chosen: GitHubUser[]) => (
            <>
              {chosen.map(user => (
                <ComboboxChip key={user.login} aria-label={user.login}>
                  {user.login}
                </ComboboxChip>
              ))}
              <ComboboxChipsInput data-testid="create-pull-request-reviewer-search" placeholder={chosen.length === 0 ? "Search by login or name" : undefined} />
            </>
          )}
        </ComboboxValue>
      </ComboboxChips>
      <ComboboxContent anchor={anchor}>
        <ComboboxEmpty>{empty}</ComboboxEmpty>
        <ComboboxList>
          {(user: GitHubUser) => (
            <ComboboxItem key={user.login} value={user}>
              <span className="font-medium">{user.login}</span>
              {user.name !== null && <span className="truncate text-muted-foreground">{user.name}</span>}
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  )
}
