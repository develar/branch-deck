package com.example.windows

import com.example.ui.ToolWindow
import kotlinx.coroutines.CoroutineScope

open class ToolWindowManagerImpl(
  val project: Project,
  internal val isNewUi: Boolean,
  private val isEdtRequired: Boolean,
  internal val coroutineScope: CoroutineScope,
) : ToolWindowManagerEx(), Disposable {
  companion object {
    fun registerLocalToolWindowForTest(project: Project, buttonManager: ToolWindowButtonManager): ToolWindowManagerImpl {
      val manager = object : ToolWindowManagerImpl(
        project = project,
        isNewUi = false,
        isEdtRequired = false,
        coroutineScope = project.coroutineScope,
      ) {
        override fun getButtonManager(toolWindow: ToolWindow): ToolWindowButtonManager = buttonManager
      }
      return manager
    }
  }

  @VisibleForTesting
  internal open fun getButtonManager(toolWindow: ToolWindow): ToolWindowButtonManager = getToolWindowPane(toolWindow).buttonManager

  @VisibleForTesting
  internal open fun getButtonManager(paneId: String): ToolWindowButtonManager = getToolWindowPane(paneId).buttonManager

  override fun dispose() {}

  private fun getToolWindowPane(paneId: String): ToolWindowPane = panes.getValue(paneId)

  fun <T> convert(value: T, into: MutableList<T>, onDone: (ToolWindow) -> Unit) {}
}

internal class ToolWindowUnregisterTest {
  fun unregister(entry: ToolWindowEntry, button: StripeButtonManager) {
    entry.stripeButton = object : StripeButtonManager by button {
      override fun remove(anchor: ToolWindowAnchor, split: Boolean) {
        button.remove(anchor, split)
      }
    }
  }

  private fun createManager(project: Project, buttonManager: ToolWindowButtonManager): ToolWindowManagerImpl {
    val manager = object : ToolWindowManagerImpl(project, isNewUi = false, isEdtRequired = false, coroutineScope = project.coroutineScope) {
      override fun getButtonManager(toolWindow: ToolWindow): ToolWindowButtonManager = buttonManager

      override fun getButtonManager(paneId: String): ToolWindowButtonManager = buttonManager
    }
    return manager
  }
}

abstract class Base<T>(open val label: String) {
  abstract fun accept(value: T, vararg tags: String)

  open fun describe(prefix: String = "", suffix: String? = null): String = prefix

  open val size: Int get() = 0

  fun fixed() {}

  private fun hidden() {}

  open fun String.decorate(times: Int): String = this
}

class Sub(override val label: String, val extra: Int) : Base<String>(label) {
  override fun accept(value: String, vararg tags: String) {}

  final override fun describe(prefix: String, suffix: String?): String {
    class Local : Runnable {
      override fun run() {}
    }
    return prefix
  }

  override val size: Int = 1

  override fun String.decorate(times: Int): String = repeat(times)

  class Nested {
    override fun toString(): String = "nested"
  }
}

enum class Level(val code: Int) : Weighted {
  LOW(1) {
    override fun weight() = 1
  },
  HIGH(2);

  open fun weight(): Int = code
}

interface Listener {
  fun changed(paths: List<String>)

  private fun log() {}
}

object Registry : Listener {
  override fun changed(paths: List<String>) {}

  open fun reset() {}
}

val quiet = object : Listener { override fun changed(paths: List<String>) {} }

class Delegating(listener: Listener) : Listener by listener {
  override fun changed(paths: List<String>) {}
}

fun afterDelegating() {}

fun localSized(): Sized {
  class Local(override val size: Int) : Sized
  return Local(1)
}

class Factory {
  companion object : Creator<Factory> {
    override fun create(): Factory = Factory()
  }
}

class Handler : Weighted, (Event) -> Unit {
  override fun invoke(event: Event) {}
}
