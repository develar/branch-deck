# UI Theme: Islands Layering, Palette and the Radius Scale

**Status:** Implemented  
**Updated:** 2026-10-25  
**Tags:** [ui, theme, shadcn, tailwind, radius, palette]

## Problem

Branch Deck runs inside a JetBrains-centric workflow, and the stock shadcn Rhea preset, its first style, looked foreign
next to IntelliJ. The preset had two problems:

- **Corners too round.** At `--radius: 0.625rem`, cards were 24px, popovers 22px, and buttons, inputs and badges 18px.
- **No shared container style.** The main window mixed three container styles:
  - shadcn `Card`;
  - a hand-rolled `CollapsibleCard` (`rounded-lg ring-1 ring-border`, no shadow);
  - a raw `rounded-lg border bg-muted` header.

  The archived branches block visibly differed from the table above it.

The first pass fixed both but kept the shadcn colours, and the window still read as a web page next to the IDE:

- **Achromatic greys.** The neutrals had chroma 0. The canvas `#F5F5F5` was so close to white that the islands needed a
  shadow, and the dark text was near-white.
- **A green accent everywhere.** Primary was Tailwind green-500, with white text on it at 2.2:1. It coloured every push
  label, every folder icon and the selection, and it was the hue of `success`, so a selected commit read as "passed".
- **Dark popovers in the island colour**, so a menu over a table was told apart only by its ring.

The goal is the look and feel of the IntelliJ Islands Light and Dark themes while staying on stock shadcn components, so a
later `npx shadcn@latest add --overwrite` keeps working. No shadcn preset or tweakcn theme comes close. The nearest stock
base colour, `zinc`, has the lightness and chroma of the IntelliJ greys, but its hue is 286, not about 265.

## Solution Overview

**Coherence over copying.** We take the proportions, the layering and the colours of Islands and express them only
through shadcn tokens. We do not copy IntelliJ pixel values into individual components. Every radius comes from one
token, every colour of the palette maps a variable of the Islands themes (`src/islands.generated.css`), and every
top-level block is the same `Card`.

## Design

### Style

The ui primitives come from the shadcn style `base-mira` (`components.json`), with the local changes each file names in
its `// local customisation` header. It was `base-rhea` until October 2026. Mira has the geometry of the IDE, Rhea had
not (measured on the same page, Islands Light, 13px Inter):

| Style | Field | Button, `sm` | A field is | Its edge on the island |
| --- | --- | --- | --- | --- |
| IntelliJ Islands | 28px | 28px, 24px | a 1px border | 1.5:1 (`control-border`) |
| `base-mira` without its fill (ours) | 28px | 28px, 24px | a 1px border | 4.6:1 (`--input`, gray-80) |
| `base-rhea` | 32px | 32px, 28px | a fill, no border | 1.2:1 (`bg-input/50`) |
| `base-nova` | 32px | 32px, 28px | a 1px border | 4.6:1 |

Two local changes apply to every primitive:

- **No text size.** The stock Mira sets 10 and 12px (`text-xs/relaxed`, `text-[0.625rem]`) on its parts. Branch Deck's
  UI text is 13px: `body` is `text-sm`, and a container sets another step of the type scale where it needs one. A chip
  (`Badge`), a key hint (`Kbd`), an `xs` button, the group label and the shortcut of a menu, the group label of a select
  and the labels of a chart keep
  `text-xs`, their step of the type scale; a 20px chip clips the descenders of 13px text. A tooltip is 13px, the UI font,
  as in the IDE. The icons of a button are one step larger than the stock for the 13px text.
- **No field fill.** Mira fills a field with `bg-input/20` (`dark:bg-input/30`) inside its border. Here `--input` is the
  border, and a field is its edge only, as in the IDE ([Palette](#palette)).

An `xs` control is 24px, the compact size of the IDE (`Button.minimumSize.compact`), not the 20px of the stock; `sm` is
24px too, the default 28px. So a branch row stays 36px around its 24px folder button (`icon-xs`), and the names and the
commit subjects under them stay where they were.

### Layering

```mermaid
flowchart TD
    W["Window canvas: body bg-canvas"] --> H["Toolbar on the canvas, the title bar: ConfigurationHeader"]
    W --> T["Branch table island: Card"]
    W --> U["Unassigned commits island: Card"]
    W --> A["Archived branches island: CollapsibleCard built on Card"]
    W --> S["Activity side panel island: Card beside the others"]
    T --> R["Rows: hover bg-muted, a translucent tint"]
    T --> P["Inner panels: rounded-lg, e.g. NestedPanel, InfoCard"]
```

- **Window canvas.** `body` is `bg-canvas` (base layer of `src/index.css`), the IntelliJ `MainWindow.background`.
- **Toolbar.** The one block on the canvas that is not an island: `ConfigurationHeader`, 40px high, is the title bar
  of the main window, as the IntelliJ main toolbar lies on the main window background
  ([Title Bar](../../spec/repository/title-bar.spec.md)). The islands scroll under it in their own container.
- **Islands.** Every other top-level block on the canvas is a `Card` (`bg-card`, `ring-foreground/5`, no shadow).
  Examples are the tables, the collapsible sections, alerts and the sub-window context cards.
- **Dark mode.** The same tokens give islands (`#191A1C`) darker than the canvas (`#26282C`), which is what Islands Dark
  does. Popovers (`#26282C`) are lighter than the island under them. There are no `dark:` overrides.
- **White fields.** `--background` stays white (the island colour in dark), so outline buttons and bordered fields
  (`bg-background border-input`) read as fields on an island and on the canvas.

### Palette

The palette comes from the Islands themes of the IDE, not from hand-copied values:

- **The source.** The IntelliJ monorepo writes `community/tools/docs-site/styles/islands.generated.css` from
  `ManyIslandsLight.theme.json` and `ManyIslandsDark.theme.json`: every palette step (`--islands-gray-70`) and every
  semantic colour of each theme (`--islands-text-link-light`, `--islands-text-link-dark`). The docs sites of the
  monorepo map the same file.
- **The copy.** `node scripts/sync-islands-theme.mjs` writes `src/islands.generated.css`: the file at a commit, with a
  header that names the repository and the commit. It reads a local monorepo checkout (`--monorepo <path> [--ref
  <rev>]` or `INTELLIJ_MONOREPO`) or intellij-community on GitHub at a pinned commit (`--github <commit>`, the path
  without `community/`). The copy is committed, so a build needs no monorepo. Never edit it; run the script again.
- **The mapping.** `src/index.css` imports the copy and maps the `-light` variables in its light `:root` block and the
  `-dark` variables in its `prefers-color-scheme: dark` block. A token names its Islands key; a palette step names a
  value outside the keys, and its comment gives the reason.
- **The check.** `src/lib/theme-contrast.test.ts` resolves the mapping in both themes and checks the pairs: text at
  4.5:1 on the island, on a popup and on the canvas, a status text on the 10% tint of its colour, the text on a fill,
  `primary` and `--control-edge` as marks at 3:1. A change of a theme that breaks a pair fails `pnpm test:unit`.

| Token | Islands key | Light | Dark |
| --- | --- | --- | --- |
| `--canvas` | `main-window-bg` (`MainWindow.background`) | `#E9EAEE` | `#26282C` |
| `--card`, `--background` | `tool-window-bg` (`ToolWindow.background`), the island | `#FFFFFF` | `#191A1C` |
| `--popover` | `popup-bg` | `#FFFFFF` | `#26282C` |
| `--foreground` | `text-default` | `#000000` | `#D1D3D9` |
| `--muted-foreground` | `text-muted` | `#5F6269` | `#9FA2A8` |
| `--border` | `separatorColor`: `dialog-border`, `popup-border` | `#E9EAEE` | `#33353B` |
| `--input` | `gray-80` | `#73767C` | `#73767C` |
| `--control-edge` | `--input`; `gray-90` | `#73767C` | `#8B8E94` |
| `--button-border` | `control-border` (`Button.startBorderColor`) | `#D1D3D9` | `#40434A` |
| `--primary` | `text-link` | `#2F5EB9` | `#71A1FE` |
| `--success` | `green-70`; `text-success` | `#2A6E47` | `#6DB083` |
| `--warning` | `yellow-70`; `text-warning` | `#875817` | `#D59637` |
| `--destructive` | `red-70`; `text-error` | `#A4414A` | `#F57E84` |
| `--info` | `purple-70`; `purple-100` | `#6C4EBB` | `#A894F6` |

The text on a fill (`*-foreground`) is `text-over-accent` (white) in light and the island in dark.

- **`muted`, `secondary` and `accent`** are one translucent tint of the foreground, 5% in light and 7% in dark
  (`--bd-tint`), as the IntelliJ hovers are (`#00000008` on a list, `#00000012` on a toolbar) and as the JetBrains genui
  mapping does. One hover then reads on a white island and on the canvas.
- **The accent** is one token, `--primary`, with no choice in the app; `--ring` follows it.

Where the tokens leave the Islands keys, and why (the ratios measured on the values above):

- **`--primary` in light is the link blue, `blue-70`, not `Button.default` (`blue-80`, `#3871E1`).** Primary is a text
  colour here too: the shown view in the toolbar, the status of a file, a selected row. `#3871E1` had 4.6:1 on the
  island, 3.8:1 on the canvas and 4.0:1 on its own 10% tint. `#2F5EB9` has 6.1:1, 5.1:1 and 5.3:1, white on it 6.1:1.
  The primary button is one step darker than the IDE's.
- **The light statuses are palette step 70, not the `text-*` step 80.** Step 80 had 4.5:1 on the island, 3.8:1 on the
  canvas and 4.0:1 on the 10% tint of a `Callout`, a `Badge` or an `Alert`. Step 70 has 6.1:1, 5.1:1 and 5.3:1 at
  least. The dark statuses stay at the `text-*` step 100: 6.8:1 on the island, 5.8:1 on a popup, 5.8:1 on the tint.
- **`--info` is the purple of the palette.** Islands has no info key, and its brand blue is the primary. It was
  GitHub's merged violet (`#8250DF`, `#AB7DF8`); the palette steps have the same hue and the contrast of the statuses.
- **Fields and toggles are edged, at 3:1 (WCAG 1.4.11).**
  - `--input` is the border of a field (`Input`, `Select`, `Textarea`, `InputGroup`, `Combobox`), as in the Mira style:
    `gray-80`, the step the docs sites of the monorepo take, not `control-border` (`gray-130`, 1.5:1 on the island). It
    has 4.6:1 on the island and in a popup in light, 3.8:1 on the island and 3.2:1 in a popup in dark.
  - `--control-edge` is the edge of a control that only its edge shows: an unchecked `Checkbox` (the edge, no fill) and
    the track of an unchecked `Switch` (the track filled with it). They lie on an island (Settings, the find bar of a
    diff), in a popup (a dialog, the review request) and on the hover of a row in a popup (the code owners of a review
    request), never on the canvas. In light it is `--input`, 4.1:1 on the hover. In dark it is one step lighter,
    `gray-90`: `gray-80` has 2.7:1 on the hover, `gray-90` 5.3:1 on the island, 4.5:1 in a popup and 3.8:1 on the hover.
  - An outline button has the border of an IDE button, `--button-border` (`control-border`), and a solid
    `bg-background`, on the canvas and on an island alike. Its label identifies it, so it needs no 3:1 edge, but on the
    canvas its white fill alone is 1.2:1, and the stock `border-border` is the colour of the canvas: the border shows the
    button there (1.3:1 on the canvas, 1.5:1 on the island in light).
- **An `Alert` mixes the tint of its tone with the card** (`color-mix(in srgb, var(--destructive) 10%, var(--card))`),
  and its description has the full tone. An alert lies on the canvas: the translucent tint over `#E9EAEE` gave its title
  3.4:1 and its description, at 90% of the tone, 3.0:1. On the card colour they have 5.3:1. A `Callout` and a `Badge`
  lie on an island or a popup and keep the translucent tint.
- **Not checked, below 4.5:1:** the soft primary of the shown view lies on the canvas (`bg-primary/10`, 4.47:1; its
  hover `bg-primary/20`, 3.9:1), and a dark `destructive` button or badge in a popup (`dark:bg-destructive/20`, 4.1:1).

### Radius scale

The Mira style names a radius step for each kind of part. The multipliers in the `@theme inline` block of
`src/index.css` put the Islands arcs on these steps. `--radius` is `0.25rem`:

| Utility | Multiplier | Value | Used by | Islands counterpart |
| --- | --- | --- | --- | --- |
| `rounded-2xl` | 2.5× | 10px | `Card`, the file cards of a diff | `Island.arc` 20, a 10px radius |
| `rounded-xl` | 2× | 8px | `Dialog` | |
| `rounded-lg` | 1.5× | 6px | `Popover`, menus, `Alert`, inner panels in app code | `Banner.arc` 12, a 6px radius |
| `rounded-md` | 1× | 4px | button, input, select, checkbox, tooltip, toast, menu items, status chip | `Button.arc` and `Component.arc` 8, a 4px radius |
| `rounded-sm` | 0.6× | 2.4px | `xs` button | |
| `rounded-full` | | | badge, switch | |

Rows are the shadcn utilities. Islands arcs are diameters, so an arc of 20 is a 10px radius. The stock Mira card is
`rounded-lg`; `card.tsx` makes it `rounded-2xl`, so an island is 10px and an inner panel (6px) stays less round.

### Diffs

A diff ([Diff View](diff-view.md)) looks as IntelliJ's diff in Islands Light and Islands Dark, unless a difference
below has its reason. It renders in the light DOM with the classes of `src/components/diff/diff-view.css`, on the
`--diff-*` tokens of `src/index.css`, which are the values of IntelliJ's schemes (`expUI_lightScheme.xml`,
`IslandSchemeDark.xml` and its parent Darcula in `DefaultColorSchemesManager.xml`):

- **Types.** IntelliJ's diff keys: inserted green, deleted grey, modified blue, conflict red. The pale form of each is
  the colour mixed 0.6 with the editor background (`TextDiffTypeImpl.getIgnoredColor`); the tokens hold the computed
  values. Split colours a block by its type on both sides, and each inner range by its own type
  (`SimpleDiffChangeUi`). The empty side of an inserted or a deleted range is a bar 2 px wide, one line high, that
  takes no width (`DiffEmptyHighlighterRenderer`). Unified and Inline colour a deleted line grey and an inserted line
  green, their words likewise (`DiffDrawUtil.createUnifiedChunkHighlighters`). A merge colours a block by its kind; a
  resolved conflict has no background, and a dotted line (2 px on, 2 px off) above and below it.
- **Pale and full.** A block with inner ranges, also an empty list of them, has pale lines and full words; a block
  without inner ranges has full lines; a whitespace-only block is pale with nothing strong.
- **Gutter.** The colour of a line fills its line number as well (`DiffLineMarkerRenderer`): no bar beside the code.
  The line numbers are a point smaller than the code, 12 px, in `LINE_NUMBERS_COLOR`.
- **Folds.** `DIFF_SEPARATORS_BACKGROUND`, with a 1-px line of `DIFF_SEPARATOR_WAVE` above and below the row. IntelliJ
  draws a squiggle across the middle; the text of our fold stands there, so the lines go to the edges.
- **Selection and find.** `SELECTION_BACKGROUND` for a selection and for the current hit, as IntelliJ's find selects
  it; `TEXT_SEARCH_RESULT_ATTRIBUTES` for the other hits, with the 1-px box of Islands Dark.
- **Kept apart from IntelliJ** ([Diff View](diff-view.md)): Split aligns its sides with filler rows instead of the
  24-px curved divider, and its sides have a 1-px divider.
- **Inline strikes its deleted parts through**, still grey and still left out of a copy. Inline has no precedent in
  IntelliJ: it puts the deleted run beside the inserted one in one line, where only the colour would tell them apart.
  A strikethrough is the convention of tracked changes, it matches the icon of the mode, and it does not depend on
  telling colours apart. Split and Unified show each side on its own line, as IntelliJ does, without it.
- **Font.** JetBrains Mono at 13 px, its real italic faces for the italic of the schemes; the row height comes from
  the font metrics and the zoom: the size of the IntelliJ editor and of the UI text beside it, at the cost of about
  8% of the columns of a split view.
- **Ligatures** are on by default, with a switch in Settings → Diffs & Merge. IntelliJ has them off by default
  (Settings → Editor → Font → Enable ligatures); on is the user's choice. It is safe in a diff: a ligature of
  JetBrains Mono keeps the width of its cells, so the arithmetic of the layout and the wraps holds; and the bound of
  a changed range inside a ligature splits the text run, so a changed character inside `!=` or `->` stays visible as
  a character of its own. `tests/e2e/commit-files/diff-font.spec.ts` measures both, `!=` to `!==` included.

The syntax colours are the IntelliJ editor schemes: the `--syntax-*` tokens of `src/index.css`, by the roles of
`src/lib/diff/islands-themes.ts`.

### Key Decisions

1. **Decision**: Change one token, `--radius`, and never radius classes in `src/components/ui/*`.
   **Rationale**: The scale stays proportional across all components and survives `shadcn add --overwrite`. The
   `// local customisation` list in the ui files does not grow.

2. **Decision**: The radius steps of the Mira style carry the Islands arcs: 4px controls, 6px popups, 10px islands.
   **Rationale**: Mira gives each kind of part its own step, so the multipliers alone reach the IntelliJ values. Only
   the `Card` changes its step (`rounded-2xl`), because the stock card is the size of a popup.

3. **Decision**: The canvas is its own token, `--canvas`, and `muted` is a translucent tint.
   **Rationale**: The IntelliJ canvas is darker than any hover on an island. A solid `muted` equal to the canvas would
   make every ghost hover on the canvas invisible and every row hover heavy. Changing `--background` instead would turn
   the fields grey.

4. **Decision**: No `Card` shadow, only the ring (`card.tsx` is locally customised).
   **Rationale**: Islands has no shadow; an island stands out by the contrast of `#E9EAEE` and white. The faint
   `ring-foreground/5` reads as a soft edge, not a line, and it keeps islands distinct in dark mode.

5. **Decision**: The exact IntelliJ greys, not a stock base colour.
   **Rationale**: `zinc` is close in lightness and chroma but warmer (hue 286 instead of 265); next to the IDE the
   difference shows. The values are tokens either way, so the ui files don't change.

6. **Decision**: The IntelliJ blue is the one primary, and `info` is purple.
   **Rationale**: One accent as in Islands, readable with white text, and no longer the hue of `success`. Blue would
   have collided with the old blue `info` (merged, landed), so `info` takes the purple of the palette, the hue GitHub
   uses for merged.

7. **Decision**: The values come from the file the monorepo generates from the Islands themes, copied by a script.
   **Rationale**: Hand-copied values drifted apart in three places (the docs sites, the GenUI catalog, Branch Deck).
   One generated file gives every consumer the same values; each consumer keeps only its mapping. The copy keeps the
   build of Branch Deck free of the monorepo.

### Rules for UI changes

- **A new top-level block is a `Card`**:
  - use `gap-0 py-0` with your own padding, `px-4 py-2.5 sm:px-6` for a header row (40px, an IntelliJ tool window
    header), as in `UnassignedCommitListCard`;
  - for a collapsible section, use `CollapsibleCard`;
  - never put a `bg-muted` or `bg-background` div directly on the canvas, because it vanishes into or clashes with the
    window.
- **Every window's top row is its title bar** (`WindowTitleBar`, [Title Bar](../../spec/repository/title-bar.spec.md)),
  on the canvas, never an island: on the left where the user is (the repository, the stack, the list and the commit,
  the request), on the right the view of the window (Split/Unified/Inline) and the actions on its object (`ActionButton`). A
  sub-window is a `WindowPage`: the row, the islands, the footer with the position and the keys only. A preview dialog
  keeps its actions in its footer. A review is a window, never a dialog: a review is a task of minutes beside the
  branch table, a dialog would cover it; a preview is a glance (Quick Look), a dialog. The actions on the object are in
  the row in every review window, the decisions on a pull request (Request Changes…, Approve and Safe Push…, Approve)
  too: what the window is for stays in one place in each tab, as the branch review's push and Safe Push do. The
  Split/Unified/Inline toggle is in the header of the block whose diffs it switches: the title bar of a window that
  is its diffs (the branch and the commit review), the end of the tabs of the pull request review, where it shows
  only with the changes and leaves the row to the decisions.
- **Window buttons** are the platform's, never a generic set: Windows' caption buttons (46×40, Segoe Fluent), GNOME's
  and KDE's as IntelliJ draws them (a 47×40 cell, its icons; the `window-button-*` tokens of `src/index.css` are their
  colours, the one exception to the semantic palette), in the order and on the sides of the desktop's layout; they dim
  in an inactive window. A Linux window is rounded as the desktop's are (12px GNOME, 8px KDE): GTK draws the shadow
  around that radius, the page clips its canvas to it (`data-window-rounded`), square when maximized, tiled or
  fullscreen.
- **A file is the same object everywhere**: its row gives the width to its name (no hover-only button reserving room,
  the owner in the hover), and its actions are one builder (`fileMenuItems`) behind the right click of every row and
  header that shows a file (one menu each, never around a diff), and ⌘C on a focused row.
- **Don't recolour a top-level `Card` or `Alert`** with `bg-muted`. A `bg-muted` override makes the island invisible on the
  canvas.
- **Nested panels inside a card** use `rounded-lg` (6px): `InfoCard`, code boxes, diff frames and warning boxes.
  `rounded-xl` is the most they get. They must stay visibly less round than the 10px island around them.
  - A section with a header inside an island (the conflict viewer and the amend preview in the expanded row) is a
    `NestedPanel` (`src/components/shared/NestedPanel.tsx`): `rounded-lg border border-border bg-card`, without the ring
    and the shadow of a `Card`, its optional header a `SectionCardHeader` row. Never a `Card` inside a `Card`.
  - A section shown both in the row and as a sub-window page is a `SectionPanel`: a `NestedPanel` in the row, a `Card`
    island on the page (`island`).
- **Never hardcode a radius** (`rounded-[12px]`, `rounded-md` on a card). To change the overall feel, change `--radius`
  and check every component.
- **Colours** use semantic tokens only, as in the Code Style section of `AGENTS.md`.
- **A colour means one thing** (statuses, chips, badges, callouts, their icons):
  - `info` (violet): already in the baseline: merged, landed, integrated;
  - `success`: approved, passed, clean;
  - `warning`: attention without a failure: blocked, outdated, a push that drops remote commits;
  - `destructive`: a failure or a conflict;
  - `primary` (blue): the user's actions and selection: Sync, the icon of a routine push, a selected or drop-target row.
    An action that repeats on every row colours only its icon, so a fully coloured label means "look here"; the folder
    icons are neutral;
  - neutral (a `secondary` badge, muted text): a count or a routine state, such as the `SectionCardHeader` count.
- **A note inside a form, a popover or a panel is a `Callout`** (`src/components/shared/Callout.tsx`), never a
  hand-rolled tinted box:
  - `tone` is `info`, `success`, `warning` or `destructive`; the box is `rounded-lg border-<tone>/20 bg-<tone>/10 p-3`;
  - a `size-4` lead icon (the tone's icon, or `icon` for a concept such as `FileDiffIcon`), the `title` in the tone's
    colour, muted details as children; a one-sentence callout passes only the `title`.
- **The state of a whole block is an `Alert`**: the repository path error, a branch that failed to sync or is blocked,
  the empty window. Its `destructive` variant is tinted like `success`, `warning` and `info`, the tint mixed with the
  card, so an alert on the canvas is an island ([Palette](#palette)); call sites add no colour overrides.
- **Code inside a sentence is an `InlineCode`** (`src/components/shared/InlineCode.tsx`, the chip of `LinkedText`), at
  the size of its text: JetBrains Mono has the x-height and cap height of Inter (0.55 and 0.73 em), so the same size is
  the same height, and a smaller one only shrinks the code. A command on its own line is a `pre` block,
  `rounded-lg bg-muted px-3 py-2 font-mono`, one step below its text (`text-xs` below `text-sm`).
- **Help of a section is a `HelpPopover` right after its title** (`src/components/shared/HelpPopover.tsx`), as the
  context help of IntelliJ: after the title, its count and note, never among the actions, where it reads as one more
  command and its popup hangs over the card edge:
  - the trigger is an `InfoIcon` (`size-3.5`, `cursor-help`) button, so never inside another button:
    `SectionCardHeader` and `CollapsibleCard` take it as `help`; `CollapsibleCard` keeps it outside its toggle, whose
    `::after` covers the rest of the row (one button, one tab stop, the whole row toggles);
  - the popup is `w-80 p-3 text-xs`, `align="start"`; it starts with a `PopoverTitle` (`text-sm`, the section's name
    as its title reads: "Archived Branches") and an optional muted `PopoverDescription` line, then 1–3 short
    paragraphs; `HelpSection` (a `PopoverSectionTitle` head) only for a second kind of content, `HelpList` for a list,
    `HelpKeys` for keys (a `dl` grid, `KbdGroup` beside the label, "Click" as text, modifiers from `@/lib/platform`);
  - a trigger of its own (the AI toggle) uses `HelpPopoverContent` with the same title; a failure titles it in
    `destructive`.
- **A popover is one anatomy**: a `PopoverTitle` (`text-sm`), the sections under a `PopoverSectionTitle` (muted
  `text-xs font-medium`: help, the Activity days, the Robots groups), a footer after `border-t border-border pt-3`.
- **A collapsible card leads with its chevron** (`ChevronRightIcon`, turning 90° open), as every disclosure of the
  app (the stack chip, the file rows, the groups of a form); `CollapsibleCard` draws it, its callers don't.
- **A dialog is a `PreviewDialogFrame`** (`src/components/shared/PreviewDialogFrame.tsx`): a header with a visible
  `text-base` title, a body that alone scrolls, a footer of key hints and `xs` buttons. `width` is `sm` for a picker,
  `md` for a question or a help text, `form` for a form with a text field and several actions (the pull request),
  `lg` (default) for a preview. Its one column is the dialog's width (`minmax(0,1fr)`), so no row widens it past the
  card; a footer that doesn't fit wraps, a text `start` beside the actions first. Escape is a key hint (`Esc Close`),
  not a button.
- **A form below a row is an `InlineForm`** (`src/components/shared/InlineForm.tsx`, `InlineInputDialog` for one input):
  - a visible `text-sm font-medium` title and, only where it adds something, a muted description;
  - the content is at most `max-w-3xl`, an input `max-w-md`; Cancel and the submit are `xs` buttons, the submit of a
    delete `destructive`;
  - a confirmation without an input passes `focusSubmit`, so Enter confirms;
  - the action runs through `withRowProcessing` (it toasts the outcome, an error through `notifyError` with its
    `errorTitle`) or `withRowProgress` (the action toasts, e.g. with Undo) in `src/stores/inline-row-action.ts`, as an
    operation the form started (`inlineSurface`, [Running Operations](operations.md)): the progress row with its busy
    state while it runs, the form again with its input after a failure of a form with an input
    (`keepFormOnError`, `keepFormOnFailure`);
  - closing gives the focus back to the opener, or to the row (`BranchContextMenuRow` is focusable).
- **Type sizes** come from the scale in `src/index.css`, one role per step. No arbitrary sizes (`text-[13px]`,
  `text-[10px]`, `text-[0.875em]`):
  - `text-xs` (12/16px): meta lines, column headers, key hints, help popovers;
  - `text-sm` (13/20px, the IntelliJ UI font size): rows, controls, labels, short text, the commit subjects of the
    lists and the hash popover body;
  - `text-md` (14/22px): reading text, the commit bodies of the commit and branch previews (`CommitMessageBody
    size="reading"`), at most `max-w-prose` wide (65ch, about 80 characters). 13px suits a row label, not paragraphs
    a user reads;
  - `text-base` (16/24px): dialog titles, also the subject in the commit preview header, the branch name in the
    branch preview header and the title of a pull request in its review (`font-medium`, as the subject of a commit
    preview); a commit as the heading of what follows it (`CommitHeading`: a commit of the branch preview, the commit
    above its diffs in a review) is `text-md`, never smaller than its body. Island titles are `text-sm font-semibold` (`SectionCardHeader`). The one exception is the Welcome card title,
    `text-base font-semibold`: it greets once, above an otherwise empty window, so it is the size of a dialog title,
    never `text-lg`.
  - A custom step takes a t-shirt name (`text-md`): `cn` (tailwind-merge) merges any other `text-*` class as a colour
    and drops it next to `text-foreground` (`src/lib/type-scale.test.ts`). A size outside the scale is a `bd-` utility
    (`bd-window-control-glyph`, the 10px Windows caption glyph; `bd-avatar-initials`, the initials in a 20px avatar
    circle).
- **Fonts**: Inter for text, JetBrains Mono for `font-mono` (hashes, paths, code), with `calt` off so `->` stays two
  characters; `body` is `antialiased`. JetBrains Mono, not the system `ui-monospace`: it is the same on every platform,
  matches Inter's metrics (SF Mono is 4% lower and exists only on macOS), and is the font of the IntelliJ editor the
  code comes from.
- **Density** is the IntelliJ list density:
  - a branch row is 36px: every cell of the branch tables has the vertical padding `--bd-cell-y` (6px) around the 24px
    folder button (`icon-sm`, the other controls of a row are 20px), which the lanes
    of the graph and the family guide span (`StackLanes`, `FamilyGuide`);
  - the dividers of the branch table group its rows: full width between the top-level units (a stack, a lone branch,
    an issue family), from the tree (`--bd-tree-x`) inside one, so a long stack reads as one block and still has a
    line per row to follow across (`bd-branch-rows`, a 1px cell background: a collapsed border can't start inside a
    row). The archived table and the lists inside a row keep `divide-y`;
  - a two-line commit row is 50px (`bd-padding-list-item`, `py-1.5`, the subject 2px above the meta line);
  - island header rows are 40px; the table header is `text-xs font-medium text-muted-foreground` without a fill;
  - the name cell starts its lane column at 1.25rem, so a lane-0 node lines up with the header text; every other cell
    keeps `px-6` or `px-4`.
- **Motion**: an expanded branch row and an open `CollapsibleCard` fade their content in (`animate-in fade-in-0
  duration-150`, opacity only, so nothing a test measures moves). Under `prefers-reduced-motion` the enter and exit
  animations and the transitions are off, those of the popups too: a menu, a popover, a tooltip or a dialog zooms by a
  variant (`data-open:animate-in`, `data-closed:animate-out`), a class `.animate-in` does not match, so the rule takes
  every class that names `animate-in` or `animate-out`. The progress indicators (`animate-spin`, `animate-carousel`,
  `animate-robot-ring` of a running Patronus robot, `animate-pulse`) keep running.
- **Live counters are `tabular-nums`** ("Pushing 45%", the fetch percent, "syncing 3/10", download bytes, "3 / 12", the
  time of a running robot "18m · ~22m"), so
  the text does not jitter while it counts. A hash inside a sentence is in the monospace font (`InlineCode plain`).
- **Icons, one per concept** (lucide):
  - `UserCheckIcon` is a review the user is asked for (the Your Turn card, its Activity entry, the Review item);
  - `CopyIcon` copies, `SettingsIcon` opens Settings (the menu item, ☰, the toolbar, "Notification Settings…", the
    hints that open a page), `ExternalLinkIcon` opens the browser, `AppWindowIcon` opens a Branch Deck window;
  - `CircleXIcon` is an error, `TriangleAlertIcon` a warning or a conflict, in the colour of its tone;
  - `GitMergeIcon` means merged and nothing else; a commit (the merge base, the merged-by line) is
    `GitCommitHorizontalIcon`;
  - message and title icons are `size-4`, the help icon `size-3.5`.
- **A `Tooltip` is not hoverable** (`disableHoverablePopup` in `src/components/ui/tooltip.tsx`): it closes when the
  pointer leaves its trigger and lets clicks through, as IntelliJ tooltips do. In the dense rows the tooltip of a
  button can open over the form of the row above; a hoverable one stayed open there and swallowed the click.
- **An icon-only button has a `Tooltip`** and an `aria-label`; a label repeated on every row names its object
  ("Expand feature-auth"). Text is a `Tooltip`, rich content a `Popover`; never a native `title`.
- **Hover descriptions**: a popover that describes on hover a trigger with a click action of its own (a push button
  and the push of an island header, a status chip that opens its pull request or robot, a landing node, the AI toggle)
  is a `HoverDescription` (`src/components/shared/HoverDescription.tsx`): the click does the action and neither pins
  nor closes the description, which follows the pointer and shows what the click changed (the progress of the push,
  then why its button is disabled; the new state of the toggle). Base UI takes a press on the trigger of a
  hover-opened popover as a toggle: within 500 ms of the open it pinned it, so it stayed after the pointer left; later
  it closed it, and the pointer that stayed on the button opened it no more. A click opens a popover only where that
  is the action: the question before a push that drops remote commits (opened by the click itself), a help icon, a
  time, the landing overview, a chip without an item. A `Tooltip` closes on the click (`closeOnClick` of Base UI) and
  opens again when the pointer comes back: the click of a button with a tooltip often opens what it would cover, a
  menu or a dialog.
- **States**:
  - empty: one muted `text-sm` line where the content would be ("No commits found"), no icon; only the empty window
    is an `Alert`;
  - loading: an inline `Spinner` (`size-4`) and "Loading …" in a muted `text-sm` line, never a centred spinner in a
    tall box;
  - copy: through `copyText` (`src/lib/clipboard.ts`); a copy button says "Copied" in place (`CopyButton`, a labelled
    button in its label: "Copy Prompt for Agent"), a menu item copies without a toast.
- **Toasts** show above an open dialog (the viewport is `z-[60]`, the dialog overlay `z-50`) and their text is
  selectable; what they say is in [UX Writing](../guidelines/ux-writing.md).
- **Anything that opens on hover or click is a button**: a help icon, a hash or a time with a popover, the AI toggle.
  Base UI triggers render a `button` by default; never a `span` trigger with `nativeButton={false}`, which the keyboard
  cannot reach.
- **A sidebar selects as an IDE list** (the pages of Settings, the `list` variant of `TabsList`): the selected page is
  `bg-primary/15`, and `bg-primary/25` while the list has the focus, without a focus ring. With `activateOnFocus` the
  focus is always on the selected page, and the window focuses it when it opens.
- **Focus ring**: `outline-none focus-visible:ring-3 focus-visible:ring-ring/30` with a radius (`rounded-sm` on inline
  text, `rounded-lg` on a row or a header), as the shadcn controls draw it.
- **`data-state` is Base UI's open/closed vocabulary** (set by hand where E2E reads it); the domain state of a component
  is `data-status` (`data-status="upToDate"` on Update Project, `data-status="force"` on an outgoing row).
- **One busy indicator per operation**: the control that started it shows the spinner and the progress (Update
  "Updating 42%" with its bar for all its steps, the spinner and "45%" on the Safe Push of the slot, as on the compact push, the island header's push of
  its stack, also of its stack branch alone, "Pushing the whole stack 45%"), the header Sync every sync, also the one an applied update waits for as its last step; the other controls of it are disabled without one, and no text beside them repeats it ("Syncing…" next to
  the spinning Sync). An action started from a menu, a key or a toast has no control left: the "⋯" of its object's row
  shows the spinner, its hover the busy state ("Unstacking tool-events…"); a button that started one and stays shows it
  in place of its icon, with the busy state as its label (`MenuItemButton surface`). A row the operation works on shows its own part (the branch a stack push pushes now, a branch
  the sync rebuilds), never the whole of it again.
- **`Spinner` has no fixed size**: inside a `Button` it is `data-icon="inline-start"` and the button sizes it, like any
  icon; a standalone spinner passes `size-4` (`size-3` in `text-xs`). Never `LoaderCircleIcon animate-spin`.
- **Type hierarchy**: weight marks titles and names, never content, so the names stand out in a long list:
  - `font-semibold`: a card title (`SectionCardHeader`, the Landed header);
  - `font-medium`: a name, the identity of a row (the branch name in `BranchNameCell`, an issue key, a landed
    branch), the name of a file above its diff, and a commit subject only where it is a title (the commit preview
    header, `CommitHeading`);
  - regular `text-foreground`: content, the commit subject of every list (`CommitSubject` in `CommitHeader`), as the
    IntelliJ Git log shows subjects;
  - `text-xs text-muted-foreground`: meta (the commit meta line, the subject next to a branch name, column headers).
- **Two text colours**: `text-foreground` for text (a commit body too) and `text-muted-foreground` for what is secondary
  (meta, directories, hashes, section labels, a label inside a message); no opacity steps between them
  (`text-foreground/85`). Islands has one secondary grey (`text-secondary`, #73767C); ours is its `text-muted`
  (#5F6269 light, #9FA2A8 dark), which keeps a 12px line readable on the dark card.
- **One look per role** in the commit and review windows (the previews, the reviews of a commit, a branch and a pull
  request, the conflict windows, the uncommitted changes):
  - a changed file is a `FilePathLabel`: the status letter, the name in the colour of its status, "← old" of a rename,
    then the directory muted in the same size, cut at its start, as IntelliJ lists a change; the header of a file above
    its diff adds `font-medium`, a tree row leaves the directory to the rows above and puts the letter in the slot of a
    folder icon;
  - the status colours are the VCS colours of IntelliJ: added `success`, deleted muted and struck through, the rest
    `primary`, the one use of `primary` that is neither an action nor a selection; a file says its status once (no badge
    beside the letter);
  - a commit as the heading of what follows it is a `CommitHeading` (above); a state with a commit in it ("“X” does not
    apply cleanly onto master", `ConflictCommitContext`) is a title of a state, `text-sm font-medium` as an
    `AlertTitle`;
  - the head of a section inside an island ("Files 4", "Commits 4", a branch of a stack in the commit column) is a
    `SectionLabel`, muted `text-xs font-medium` with its count, as `PopoverSectionTitle`;
  - line counts are coloured (`text-success`, `text-destructive`) for one file wherever it is listed and for a commit
    or a pull request where it is the heading (the preview headers, `CommitHeading`, the chips of a pull request); a
    row of a list and a sum (a directory, a section) keeps them muted;
  - the time of a commit is absolute: the meta line says it with the committer time on hover (`TimeWithPopover`), a
    one-line list (the commits of a pull request) says it short (`formatCompactTimestamp`: "Today
    2:32 PM", "Yesterday", "Oct 3"); only the age of an event is relative ("waiting 2 days", "landed 2 hours ago");
  - a hash is `font-mono` and muted, in a window title too (`WindowTitle` part `{ text, mono: true }`);
  - every sub-window title is a `WindowTitle` in `text-sm`, the last part in the foreground ("Your Turn › 2 of 5").
- **Spacing**:
  - islands are 8px apart and 8px from the window edges, as the 6px Islands gap on the spacing scale: `gap-2` in
    `BranchCreator` and `WindowPage`, a full-width `Container` with `px-2` (`pb-2` under the toolbar of the main
    window), `px-2 pb-2` under the row of a sub-window;
  - islands take the full window width, as IntelliJ tool windows do; the one block beside them is a side panel.
- **A side panel** (Activity, [Activity](../../spec/repository/activity.spec.md#side-panel)) is an island of its own
  to the right of the islands, under the toolbar, which stays the full width: a `Card` with a 40px header (the title,
  its actions, Close), 8px from the islands, the window edge and the bottom. The 8px gap is its divider
  (`ResizableHandle`, transparent), dragged to a width that is kept (`useDefaultLayout`, as the sidebar of a review).
  It opens only where the islands keep their minimum beside it (`MIN_ISLANDS_WIDTH`, 768 CSS px, plus the panel's own
  320 px); without the room, what would open it opens its window instead, never a squeezed table.

- **The views of the main window** ([Main Views](../../spec/repository/main-views.spec.md)): the switcher after the
  repository in the toolbar, two `xs` ghost buttons, the shown one soft primary (a tint, no surface on the canvas);
  each view its own scroller of islands under the toolbar, both kept mounted.
- **Charts** (Landing, [Main Views](../../spec/repository/main-views.spec.md#user-experience)): shadcn Charts on
  Recharts (`src/components/ui/chart.tsx`, lazy), the series in the chart palette `--chart-1…4` (dataviz reference
  slots and a neutral for an end without an outcome, validated for colour-blind separation on both cards; never the
  status tokens for a series), one colour per entity across the charts (the user in the ink of the text, never in the
  hue of another entity; everyone else as context in `--chart-context`, a lighter step of the blue), the week of now
  muted and named "This week", bars ≤ 24px with a 2px card gap and a 4px rounded data end, hairline solid grid, one
  key component (`ChartKey`, left-aligned, from two series), values in the text tokens, a table twin per chart, the
  ranked lists of the failures in one form (`RankedList`). The islands are as tall as their content: the figures on
  the full width in a subgrid of five rows, the charts in two columns. A figure's comparison is the one place for a
  status colour, with its arrow and words.

### Branch columns

The branch table and every stack island are cards of their own, each a `<table>`, read under the one row of column
heads of the table. So their columns must stand at the same place in every card, and the island header's actions over
the rows' actions. A column is as wide as its widest content in any card; no cell holds a place for content that no row
shows.

- **Tracks**: each kind of item in a column is a track: `commits`; inside Status `state` (the robot or the status
  text), `pr`, `landed`; inside Actions `push`, `step`, `more` (`BRANCH_TRACKS`, `src/lib/branch-columns.ts`). Each item
  is a `ColumnTrack`: an outer box at least `--bd-track-<kind>` wide, its content measured at its natural width.
- **One measurement**: `ColumnRegistry` (one per view, `BranchColumnsRoot` around the table and its islands) observes
  the content of every item with one `ResizeObserver` and writes the widest per track as `--bd-track-<kind>` on the
  root. The observer reports a frame's sizes before it paints, so the columns are in place in every painted frame; a
  check of the alignment polls. A collapse that removes rows writes once (a microtask).
- **Empty is free**: a track that no row of any card fills is 0 wide, and so is its gutter (`--bd-gap-<kind>`, 8px
  while the track has content): a view without Patronus has no step column, a pushed view no push column.
- **Caps**: `state` and `step` are at most 10rem, `landed` 12rem; their text truncates, the hover says it. The labels of
  the steps fit (`next-step.spec.md`); the cap is a backstop.
- **Late facts move no column**: the answers of the remote come after the first rows (the pull request chips, the
  steps, the robots), so until they are in ([Facts without an answer](../../spec/branch/next-step.spec.md#facts-without-an-answer))
  each track is at least as wide as once they were in the last time (`ColumnRegistry.setFloors`, the `tracks` of
  `settledLayout` in the repository state, `useSettledLayout`): a start or a switch back stands where the settled view
  of the last time stood, and the answers fill the places they had. The widths are kept as laid out, fractions too: a
  floor rounded up stands a fraction wider than the content it holds the place of, and a track rounded up moves its
  neighbours by that fraction a frame after the content, when the report of the observer comes. The floors go in the
  commit that renders the settling answer, the items read as laid out then (`setFloors(null)`), not as the observer
  last reported them, so their drop moves nothing for a frame either. Once they are in, the floors go: the view follows
  its content (a track that has less to show than the last time narrows then, once) and keeps its widths for the next
  start. A repository's first start has no floors; within a window a repository settles once, so a late fact after it
  (a new branch waiting for its readiness) moves only what it changes. Your Turn above the table holds its height the
  same way ([Review Requests](../../spec/repository/review-requests.spec.md#card)), the second header line of a stack
  island its line (`stackLines` of `settledLayout`, [Stack Island](../../spec/branch/stack-island.spec.md)), the
  sections of the branch preview a muted line (`PendingSection`) and its footer push, like the push of the review
  window's title bar, its place, disabled with why ([Branch Preview](../../spec/branch/branch-preview.spec.md)), and the
  unassigned commits card renders with the rows (no lazy chunk: one came seconds later under load and moved the table).
- **Live values hold their own width**: a running robot is at least `13ch`, the compact push `min-w-16` (`⟳ 100%`),
  the Safe Push of a slot its width at rest while its start runs, so
  a tick of a time or a percent changes no column. A minimum lives on the element that changes, never in the column:
  a column minimum costs its width where nothing shows (the 328px Actions column of October 2026 was two of them).
- **Labels**: the label of a group's column (Status, Actions) is the minimum of the group's items in every row
  (`ColumnLabel`, `--bd-group-<group>`), so a narrow group doesn't widen the table's column alone; Commits labels its
  track itself.
- **Per card**: the lane column of the stack graph stays the card's own (`--bd-lane-count`); the archived table keeps
  its own columns (Status and Integrated mean other things) and only its "⋯" lines up, 16px from the edge.
- **Why measured, not CSS subgrid**: one grid with every card, table and row a subgrid lines the columns up without
  code, but its track sizing reruns over every item of every card on any change. At 141 rows (`pnpm perf:table`,
  `tests/perf/branch-table.perf.ts`, the `branch_table_large` template) a text change cost 8.5–12.7 ms in WebKit, the
  engine of the app on macOS, against ~1 ms for the tables, and a resize 15 ms against 3.8. The tables with measured
  tracks cost what the tables did: 0.9 ms for a tick, 4 ms for a text that changes its width, plus one relayout when it
  changes the widest item of a track.

### Verification

Every visual change gets before and after screenshots in light and dark:

- build `dist/` and start `test-server`;
- write a throwaway Playwright spec with `deviceScaleFactor: 2`, `colorScheme` set to both, and the fixtures
  `setupRepo("archived_branches" | "unassigned" | "conflict_branches")`;
- check that nothing vanishes on the canvas and that nested panels stay less round than the islands.

A change the README images show (the main window, the review window) regenerates them: `pnpm screenshots:readme`
(`tests/screenshots/readme.shot.ts`, WebKit at 2x) rewrites `docs/screenshots/`.

Class changes also change the HTML snapshots. Run `pnpm e2e-up`, then review that the snapshot diff only touches classes.

## Alternatives Considered

- **Copy IntelliJ values per component** (4px buttons, 10px cards):
  - Why not: it means per-component overrides in `src/components/ui/*`, which are lost on every `--overwrite`, and the
    proportions drift apart over time.
- **Stay on the Rhea style** (`base-rhea`, the first style):
  - Why not: its 32px controls are larger than the 28px of the IDE, and its fields are a fill of 1.2:1 without a border;
    the IDE draws a field by its border.
- **The Nova style** (`base-nova`):
  - Why not: it has the borders, but its controls are 32px like Rhea's. Mira has the 28px and 24px of the IDE.
- **shadcn preset radius "small"** (~0.45rem):
  - Why not: it still gives 19px cards and 13px controls, far rounder than Islands.
- **Stock `zinc` base colour** (`baseColor: zinc`):
  - Why not: warmer than the IntelliJ greys (hue 286 instead of 265), and hand-written tokens cost nothing more.
- **Keep the green accent and use it less**:
  - Why not: green stays the hue of `success`, and white on it stays below AA.
- **A choice of accent palettes** (the former "Theme…" dialog, inherited from Nuxt UI: the IntelliJ blue and the
  Tailwind palettes):
  - Why not: the contrast of the tokens is checked for one accent, as Islands has one; a picked palette could fall
    below AA or take the hue of a status colour (green `success`, violet `info`).

## References

- Tokens and scale: `src/index.css` (`--radius`, the `@theme inline` radius multipliers, the palette, `body` in the base
  layer).
- Palette source: `src/islands.generated.css`, written by `scripts/sync-islands-theme.mjs` from
  `community/tools/docs-site/styles/islands.generated.css` of the IntelliJ monorepo
  (`community/tools/docs-site/scripts/islandsTheme.ts` writes it); the contrast check `src/lib/theme-contrast.test.ts`.
- Islands containers:
  - `src/components/ui/card.tsx`;
  - `src/components/shared/CollapsibleCard.tsx`;
  - `src/components/shared/NestedPanel.tsx`;
  - `src/components/shared/Callout.tsx`, `src/components/shared/HelpPopover.tsx`, `src/components/shared/InlineCode.tsx`,
    `src/components/shared/PreviewDialogFrame.tsx`;
  - `src/components/repository/ConfigurationHeader.tsx`;
  - `src/components/branch/BranchCreator.tsx`.
- IntelliJ Islands theme: `community/platform/platform-resources/src/themes/islands/ManyIslandsLight.theme.json` and
  `ManyIslandsDark.theme.json` in the IntelliJ monorepo (`Island.arc`, `Button.arc`, `Component.arc`, `Banner.arc`,
  `main-window-bg`, `tool-window-bg`, the `gray-*` and `blue-*` palette).
- JetBrains genui, the IntelliJ theme mapped to shadcn tokens:
  `plugins/air/frontend/genui/jcef/web-bundle/src/host/style-variables.ts` and `src/catalog/styles/catalog.css`.
- Related docs: [e2e-testing.md](e2e-testing.md) for the snapshot workflow.
