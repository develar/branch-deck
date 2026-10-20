package com.acme.notes.editor

/** Keeps the first visible paragraph in place when the file changes on disk and the editor reloads it */
class ScrollKeeper(private val editor: Editor) {
  fun reload(text: String) {
    val anchor = editor.firstVisibleParagraph()
    editor.setText(text)
    anchor?.let { editor.scrollTo(it.id, offset = it.offset) }
  }
}
