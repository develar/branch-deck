package com.acme.notes.sync

/** Who else has the document open, with the colour their cursor is drawn in */
data class Collaborator(val name: String, val color: Int, val paragraphId: String?)

class Presence(private val server: SyncServer) {
  suspend fun collaborators(documentId: String): List<Collaborator> = server.presence(documentId).sortedBy { it.name }
}
