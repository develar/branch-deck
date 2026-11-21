
/** The words of three versions of a paragraph side by side: the base, mine and theirs */
internal fun diffWords(base: String, mine: String, theirs: String): List<WordChange> {
  val (myWords, theirWords) = mine.split(' ') to theirs.split(' ')
  return base.split(' ').mapIndexed { index, word ->
    WordChange(word, myWords.getOrElse(index) { "" }, theirWords.getOrElse(index) { "" })
  }
}

internal data class WordChange(val base: String, val mine: String, val theirs: String)
