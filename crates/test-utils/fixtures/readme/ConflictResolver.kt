package com.acme.notes.sync

/** Merges two edits of the same paragraph word by word; where both changed the same words, the later edit wins */
class ConflictResolver {
  fun merge(mine: Edit, theirs: Edit): Edit {
    val words = diffWords(mine.base, mine.text, theirs.text)
    val text = words.joinToString(" ") { change ->
      when {
        change.mine == change.theirs -> change.mine
        change.mine == change.base -> change.theirs
        change.theirs == change.base -> change.mine
        mine.time > theirs.time -> change.mine
        else -> change.theirs
      }
    }
    return theirs.copy(text = text, time = maxOf(mine.time, theirs.time))
  }
}
