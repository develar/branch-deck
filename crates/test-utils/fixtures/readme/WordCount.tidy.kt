package com.acme.notes.editor

/** Counts the words of a document as the status bar shows them */
class WordCount {
  fun count(text: String): Int {
    val words = text.split(' ', '\n', '\t').filter { it.isNotBlank() }
    return words.size
  }

  fun readingMinutes(text: String, wordsPerMinute: Int = 200): Int =
    (count(text) + wordsPerMinute - 1) / wordsPerMinute
}
