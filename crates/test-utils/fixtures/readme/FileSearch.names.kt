package com.acme.notes.search

/** Finds the notes whose path contains the query, those whose file name contains it first */
class FileSearch(private val index: NoteIndex) {
  fun find(query: String): List<Note> =
    index.all()
      .filter { it.path.contains(query, ignoreCase = true) }
      .sortedByDescending { it.name.contains(query, ignoreCase = true) }
}
