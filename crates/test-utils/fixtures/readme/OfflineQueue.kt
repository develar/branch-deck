package com.acme.notes.sync

/** The edits made without a connection, in order, written to disk so a restart of the app keeps them */
class OfflineQueue(private val file: QueueFile) {
  fun add(edit: Edit) {
    file.append(edit.encode())
  }

  fun drain(): List<Edit> {
    val edits = file.readAll().map(Edit::decode)
    file.clear()
    return edits
  }
}
