package com.acme.notes.sync

/** Uploads the edits queued offline as soon as the connection is back, in the order they were made */
class ConnectionWatcher(private val queue: OfflineQueue, private val server: SyncServer) {
  suspend fun onReconnect(documentId: String) {
    val edits = queue.drain()
    if (edits.isNotEmpty()) {
      server.upload(documentId, edits)
    }
  }
}
