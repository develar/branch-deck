import type { ThemeRegistration } from "@pierre/diffs"

/** The colours of a syntax theme: the IntelliJ New UI editor schemes (Light, Dark), by role */
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

const LIGHT: EditorPalette = {
  foreground: "#080808",
  keyword: "#0033B3",
  string: "#067D17",
  escape: "#0037A6",
  number: "#1750EB",
  comment: "#8C8C8C",
  functionDeclaration: "#00627A",
  field: "#871094",
  annotation: "#9E880D",
  tag: "#0033B3",
  attribute: "#174AD4",
}

const DARK: EditorPalette = {
  foreground: "#BCBEC4",
  keyword: "#CF8E6D",
  string: "#6AAB73",
  escape: "#CF8E6D",
  number: "#2AACB8",
  comment: "#7A7E85",
  functionDeclaration: "#56A8F5",
  field: "#C77DBB",
  annotation: "#B3AE60",
  tag: "#D5B778",
  attribute: "#BABABA",
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

function islandsTheme(name: string, type: "light" | "dark", palette: EditorPalette): ThemeRegistration {
  return {
    name,
    type,
    // the background comes from the card behind the diff (src/index.css)
    colors: { "editor.foreground": palette.foreground, "editor.background": type === "light" ? "#FFFFFF" : "#191A1C" },
    tokenColors: tokenColors(palette),
  }
}

export const ISLANDS_LIGHT_THEME = islandsTheme("bd-islands-light", "light", LIGHT)
export const ISLANDS_DARK_THEME = islandsTheme("bd-islands-dark", "dark", DARK)
