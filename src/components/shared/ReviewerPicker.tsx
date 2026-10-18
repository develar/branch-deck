import { SearchIcon } from "lucide-react"
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
import type { ReviewerSearch } from "@/hooks/useReviewerSearch"
import { MIN_SEARCH_LENGTH } from "@/hooks/useReviewerSearch"
import type { GitHubUser } from "@/lib/bindings"
import { isLogin, searchResults } from "@/lib/create-pull-request"

interface ReviewerPickerProps {
  /** Names the chips (`<prefix>-reviewers`) and the search (`<prefix>-reviewer-search`) for the tests */
  testIdPrefix: string
  reviewers: GitHubUser[]
  setReviewers: (reviewers: GitHubUser[]) => void
  search: ReviewerSearch
  /** The user, never offered: GitHub refuses a review request of the author */
  viewerLogin: string | null
  disabled: boolean
  placeholder?: string
}

/** The chosen reviewers as chips, and the search of more: the Create Pull Request dialog and Request Review… */
export function ReviewerPicker({ testIdPrefix, reviewers, setReviewers, search, viewerLogin, disabled, placeholder = "Search by login or name" }: ReviewerPickerProps) {
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
      <ComboboxChips ref={anchor} data-testid={`${testIdPrefix}-reviewers`}>
        {/* a search, as every search of a list: the icon first */}
        <SearchIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        <ComboboxValue>
          {(chosen: GitHubUser[]) => (
            <>
              {chosen.map(user => (
                <ComboboxChip key={user.login} aria-label={user.login}>
                  {user.login}
                </ComboboxChip>
              ))}
              <ComboboxChipsInput data-testid={`${testIdPrefix}-reviewer-search`} placeholder={chosen.length === 0 ? placeholder : undefined} />
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
