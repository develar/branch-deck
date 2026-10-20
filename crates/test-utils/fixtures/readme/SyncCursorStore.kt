package com.acme.notes.sync

import java.util.prefs.Preferences

/** The position in the edit log of the server up to which a document is synced, kept between the starts of the app */
class SyncCursorStore(private val preferences: Preferences) {
  fun lastCursor(documentId: String): Long = preferences.getLong(key(documentId), 0)

  fun save(documentId: String, cursor: Long) {
    preferences.putLong(key(documentId), cursor)
  }

  private fun key(documentId: String) = "sync.cursor.$documentId"
}
