package com.acme.notes.search

/** Finds the notes whose path contains the query */
class FileSearch(private val index: NoteIndex) {
  fun find(query: String): List<Note> = index.all().filter { it.path.contains(query, ignoreCase = true) }
}
