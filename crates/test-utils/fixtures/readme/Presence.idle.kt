package com.acme.notes.sync

import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes

/** Who else has the document open, with the colour their cursor is drawn in */
data class Collaborator(val name: String, val color: Int, val paragraphId: String?, val idleFor: Duration = Duration.ZERO)

class Presence(private val server: SyncServer) {
  suspend fun collaborators(documentId: String): List<Collaborator> =
    server.presence(documentId).filter { it.idleFor < 10.minutes }.sortedBy { it.name }
}
