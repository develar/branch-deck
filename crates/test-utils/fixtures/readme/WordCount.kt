package com.acme.notes.editor

/** Counts the words of a document as the status bar shows them */
class WordCount {
  fun count(text: String): Int {
    val parts = text.split(' ', '\n', '\t').filter { it.isNotBlank() }
    return parts.size
  }

  fun readingMinutes(text: String, wordsPerMinute: Int = 200): Int = (count(text) + wordsPerMinute - 1) / wordsPerMinute
}
