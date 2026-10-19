import { bundledLanguages, bundledLanguagesAlias } from "shiki"
import type { LanguageRegistration } from "shiki"
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
  const modules = new Map<string, Promise<LanguageRegistration[]>>()
  const registrationsOf = (language: string) => {
    let module = modules.get(language)
    if (module === undefined) {
      module = importOf(language)!().then(imported => imported.default)
      modules.set(language, module)
    }
    return module
  }

  /*
   * A grammar embeds others eagerly (they come in its module) or lazily: Markdown its front matter (YAML) and its
   * fenced blocks, Vue and Svelte their blocks. Shiki rebuilds a grammar when one it embeds lazily loads after it, and
   * the engine keeps the states of the old one; without it, the include stays empty (the front matter of Markdown is a
   * paragraph). So a grammar loads after the bundled grammars it embeds, theirs included.
   */
  const load = async (language: string) => {
    const waves: LanguageRegistration[][] = []
    const seen = new Set([language])
    let wave = [language]
    while (wave.length > 0) {
      const loaded = await Promise.all(wave.map(registrationsOf))
      waves.push(...loaded)
      wave = loaded.flatMap(registrations => registrations.flatMap(registration => registration.embeddedLangsLazy ?? []))
      wave = [...new Set(wave)].filter(id => !seen.has(id) && importOf(id) !== undefined)
      wave.forEach(id => seen.add(id))
    }
    // the deepest first, the grammar itself last; one already loaded is left as it is
    for (const registrations of waves.toReversed()) {
      await highlighter.loadLanguage(registrations)
    }
    return true
  }

  return {
    grammar: async language => {
      if (importOf(language) === undefined) {
        return null
      }
      let loaded = loading.get(language)
      if (loaded === undefined) {
        loaded = load(language)
        loading.set(language, loaded)
      }
      await loaded
      return highlighter.getLanguage(language)
    },
    styleOf: metadata => (roles[EncodedTokenMetadata.getForeground(metadata)] ?? 0) | (EncodedTokenMetadata.getFontStyle(metadata) << FONT_STYLE_SHIFT),
  }
}

/** The import of a bundled language by its id or an alias; undefined for one the build leaves out */
function importOf(language: string): (typeof bundledLanguages)[keyof typeof bundledLanguages] | undefined {
  return Object.hasOwn(bundledLanguages, language)
    ? bundledLanguages[language as keyof typeof bundledLanguages]
    : bundledLanguagesAlias[language as keyof typeof bundledLanguagesAlias]
}
