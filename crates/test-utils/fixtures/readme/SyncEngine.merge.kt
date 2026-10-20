package com.acme.notes.sync

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

/** Uploads the local edits of a document and applies the edits the other devices made */
class SyncEngine(
  private val server: SyncServer,
  private val store: DocumentStore,
  private val cursors: SyncCursorStore,
  private val resolver: ConflictResolver = ConflictResolver(),
) {
  suspend fun sync(documentId: String): SyncResult {
    val local = store.pendingEdits(documentId)
    // only the edits after the last sync: a long document no longer downloads its whole history
    val remote = server.fetchEdits(documentId, since = cursors.lastCursor(documentId))
    for (edit in remote.edits) {
      val mine = local.firstOrNull { it.paragraphId == edit.paragraphId }
      store.apply(if (mine == null) edit else resolver.merge(mine, edit))
    }
    server.upload(documentId, local)
    store.markSynced(documentId, local)
    cursors.save(documentId, remote.cursor)
    return SyncResult(uploaded = local.size, downloaded = remote.edits.size)
  }

  fun changes(documentId: String): Flow<SyncResult> = flow {
    while (true) {
      emit(sync(documentId))
      server.awaitChanges(documentId)
    }
  }
}

data class SyncResult(val uploaded: Int, val downloaded: Int)
