import { bundledLanguages, bundledLanguagesAlias } from "shiki"
import { createHighlighterCore } from "shiki/core"
import { createJavaScriptRegexEngine } from "shiki/engine/javascript"
import { EncodedTokenMetadata } from "shiki/textmate"
import type { GrammarSource } from "@/lib/diff/highlight/engine"
import { FONT_STYLE_SHIFT } from "@/lib/diff/highlight/protocol"
import { ROLE_THEME, roleColor, SYNTAX_ROLES } from "@/lib/diff/islands-themes"

/**
 * The grammars of the bundled languages with the theme of roles (docs/design/diff-view.md, "One theme of roles"), on
 * Shiki's JavaScript regex engine: no WebAssembly. A grammar loads with the first blob of its language.
 */
export async function createGrammarSource(): Promise<GrammarSource> {
  const highlighter = await createHighlighterCore({ themes: [ROLE_THEME], langs: [], engine: createJavaScriptRegexEngine() })
  const { colorMap } = highlighter.setTheme(ROLE_THEME.name!)
  const roleByColor = new Map(Array.from({ length: SYNTAX_ROLES.length + 1 }, (_, role) => [roleColor(role), role]))
  // the index of the colour map in a token's metadata gives its role; another colour is the default foreground
  const roles = colorMap.map(color => roleByColor.get(color.toUpperCase()) ?? 0)
  const loading = new Map<string, Promise<boolean>>()
  return {
    grammar: async language => {
      const input = Object.hasOwn(bundledLanguages, language)
        ? bundledLanguages[language as keyof typeof bundledLanguages]
        : bundledLanguagesAlias[language as keyof typeof bundledLanguagesAlias]
      if (input === undefined) {
        return null
      }
      let loaded = loading.get(language)
      if (loaded === undefined) {
        loaded = highlighter.loadLanguage(input).then(() => true)
        loading.set(language, loaded)
      }
      await loaded
      return highlighter.getLanguage(language)
    },
    styleOf: metadata => (roles[EncodedTokenMetadata.getForeground(metadata)] ?? 0) | (EncodedTokenMetadata.getFontStyle(metadata) << FONT_STYLE_SHIFT),
  }
}
