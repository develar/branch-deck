import type { ThemeRegistration } from "shiki"

/**
 * The colours of a syntax theme by role. The colours of the IntelliJ New UI editor schemes (Light, Dark) are the
 * `--syntax-*` tokens of `src/index.css`; `ROLE_THEME` gives each role an id instead of a colour
 */
interface EditorPalette {
  foreground: string
  keyword: string
  string: string
  escape: string
  number: string
  comment: string
  functionDeclaration: string
  field: string
  annotation: string
  tag: string
  attribute: string
}

/** TextMate scopes by palette role; a later rule wins over an earlier one for the same token */
function tokenColors(palette: EditorPalette): ThemeRegistration["tokenColors"] {
  return [
    { scope: ["comment", "punctuation.definition.comment", "string.quoted.docstring"], settings: { foreground: palette.comment, fontStyle: "italic" } },
    { scope: ["string", "punctuation.definition.string", "markup.inline.raw", "markup.fenced_code"], settings: { foreground: palette.string } },
    { scope: ["constant.character.escape", "constant.other.placeholder", "constant.character.format.placeholder"], settings: { foreground: palette.escape } },
    { scope: ["constant.numeric"], settings: { foreground: palette.number } },
    {
      scope: ["keyword", "storage.type", "storage.modifier", "constant.language", "variable.language", "support.type.primitive", "markup.heading"],
      settings: { foreground: palette.keyword },
    },
    // operators and accessors are plain text in IntelliJ
    { scope: ["keyword.operator", "punctuation"], settings: { foreground: palette.foreground } },
    { scope: ["entity.name.function", "support.function"], settings: { foreground: palette.functionDeclaration } },
    // IntelliJ colours a declaration, not a call
    { scope: ["entity.name.function.call", "meta.function-call entity.name.function", "meta.function-call support.function"], settings: { foreground: palette.foreground } },
    {
      scope: [
        "variable.other.property",
        "variable.other.member",
        "variable.other.constant.property",
        "support.variable.property",
        "support.type.property-name",
        "variable.other.enummember",
      ],
      settings: { foreground: palette.field },
    },
    {
      scope: [
        "entity.name.type.annotation",
        "storage.type.annotation",
        "meta.annotation",
        "punctuation.definition.annotation",
        "meta.decorator",
        "entity.name.function.decorator",
        "punctuation.decorator",
      ],
      settings: { foreground: palette.annotation },
    },
    { scope: ["entity.name.tag", "punctuation.definition.tag"], settings: { foreground: palette.tag } },
    { scope: ["entity.other.attribute-name"], settings: { foreground: palette.attribute } },
    { scope: ["markup.bold", "markup.heading"], settings: { fontStyle: "bold" } },
    { scope: ["markup.italic"], settings: { fontStyle: "italic" } },
  ]
}

/**
 * The roles of a token after the default foreground (role 0), in the order of their ids; the class of a role is
 * `t-<role>`, its colours `--syntax-<role>` of `src/index.css`
 */
export const SYNTAX_ROLES = ["keyword", "string", "escape", "number", "comment", "functionDeclaration", "field", "annotation", "tag", "attribute"] as const

/** The colour that stands for a role in `ROLE_THEME`: no real colour, an id */
export function roleColor(role: number): string {
  return `#0000${(role + 1).toString(16).padStart(2, "0").toUpperCase()}`
}

/**
 * One theme of roles for the highlight worker (docs/design/diff-view.md, "One theme of roles"): the scope rules of the
 * Islands themes, each colour a role id. One tokenization serves both schemes; the colours are CSS.
 */
export const ROLE_THEME: ThemeRegistration = {
  name: "bd-syntax-roles",
  type: "light",
  colors: { "editor.foreground": roleColor(0), "editor.background": "#FFFFFF" },
  tokenColors: tokenColors({
    foreground: roleColor(0),
    ...(Object.fromEntries(SYNTAX_ROLES.map((role, index) => [role, roleColor(index + 1)])) as Omit<EditorPalette, "foreground">),
  }),
}
