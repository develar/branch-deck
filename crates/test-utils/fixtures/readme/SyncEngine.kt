package com.acme.notes.sync

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

/** Uploads the local edits of a document and applies the edits the other devices made */
class SyncEngine(
  private val server: SyncServer,
  private val store: DocumentStore,
) {
  suspend fun sync(documentId: String): SyncResult {
    val local = store.pendingEdits(documentId)
    val remote = server.fetchEdits(documentId, since = 0)
    for (edit in remote) {
      store.apply(edit)
    }
    server.upload(documentId, local)
    store.markSynced(documentId, local)
    return SyncResult(uploaded = local.size, downloaded = remote.size)
  }

  fun changes(documentId: String): Flow<SyncResult> = flow {
    while (true) {
      emit(sync(documentId))
      server.awaitChanges(documentId)
    }
  }
}

data class SyncResult(val uploaded: Int, val downloaded: Int)
