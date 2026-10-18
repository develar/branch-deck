// The words of a search field: every one must be in one of the searched texts, ignoring case

/** The words of `text`, lower-cased; none for a blank text */
export function searchWords(text: string): string[] {
  return text
    .toLowerCase()
    .split(/\s+/)
    .filter(word => word !== "")
}

/** Whether every word is in one of the texts, ignoring case; true without words */
export function matchesAllWords(texts: readonly string[], words: readonly string[]): boolean {
  if (words.length === 0) {
    return true
  }
  const lowered = texts.map(it => it.toLowerCase())
  return words.every(word => lowered.some(it => it.includes(word)))
}
