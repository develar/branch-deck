import { INITIAL } from "shiki/textmate"
import type { GrammarSource } from "@/lib/diff/highlight/engine"
import { createGrammarSource } from "@/lib/diff/highlight/grammars"

/**
 * The timeout of the work of a grammar warm-up: the first load of a grammar under a loaded test run (cargo and the E2E
 * suite beside it) takes seconds, Markdown's most, which loads every grammar its fenced blocks embed, and so does the
 * first line a grammar tokenizes, which compiles its patterns
 */
export const GRAMMAR_WARM_UP_TIMEOUT = 60_000

/** Lines with the common scopes of code: a declaration, a call, a string, a number, a comment */
const WARM_UP_LINES = ['@A class B(c: Int) { fun d() = e("f", 1) } // g', "import x.y; /* z */ const w = 'v' + 2"]

/**
 * The grammar source of the highlight worker with `languages` loaded and their patterns compiled by a few lines of
 * code: a test file warms the grammars its tests use in its `beforeAll` (with `GRAMMAR_WARM_UP_TIMEOUT`), so the 5 s of
 * a test time only its own work
 */
export async function warmGrammarSource(languages: readonly string[]): Promise<GrammarSource> {
  const source = await createGrammarSource()
  for (const grammar of await Promise.all(languages.map(language => source.grammar(language)))) {
    let state = INITIAL
    for (const line of WARM_UP_LINES) {
      state = grammar?.tokenizeLine2(line, state).ruleStack ?? state
    }
  }
  return source
}
