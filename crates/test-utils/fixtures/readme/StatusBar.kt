package com.acme.notes.editor

/** The bar at the bottom of the window: the words of the document and the state of the sync */
class StatusBar(private val view: StatusView) {
  fun update(words: Int, synced: Boolean) {
    view.left = "$words words"
    view.right = if (synced) "Synced" else "Not synced"
  }
}
