package com.example.windows

import javax.inject.Inject

open class ToolWindowManagerImpl<T>(
  val project: Project,
  internal val isNewUi: Boolean = true,
  isEdtRequired: Boolean = true,
  vararg val ids: String,
  value: T?,
)

class Hidden private constructor(val size: Int) {
  constructor(name: String, vararg rest: Int) : this(name.length)

  private constructor() : this(0)

  internal constructor(first: Long, second: Long = 1) : this(first.toInt()) {
    reset()
  }

  fun reset() {}
}

class Injected @Inject internal constructor(project: Project)

class Implicit

private class Private

abstract class OnlySecondary {
  constructor(size: Int)

  constructor(name: String) : super()
}

interface Listener

object Registry

class Factory {
  companion object
}

enum class Level(val code: Int) {
  LOW(1),
  HIGH(2);

  constructor() : this(0)
}

annotation class Marker(val value: String)

typealias Managers = List<ToolWindowManagerImpl<String>>

class Outer {
  init {
    class InInit {
      constructor(size: Int)
    }
  }

  fun local() {
    class InFunction(size: Int) {
      constructor() : this(0)
    }
  }
}

class Sub : ToolWindowManagerImpl<String>(project, isEdtRequired = false, value = null), Runnable by task

class Qualified : com.example.windows.ToolWindowManagerImpl<String>(project, false, value = "x")

fun createManager(project: Project): Any {
  val manager = object : ToolWindowManagerImpl<String>(project, isNewUi = false, isEdtRequired = false, value = null) {
    override fun toString() = "test"
  }
  val inLambda = run { object : Base(-1) {} }
  @Suppress("UNCHECKED_CAST")
  val one = object : Base(1), Runnable { override fun run() {} }
  return Hidden("x", 1, 2)
}

class Box<T>(val value: T) {
  constructor(list: List<T>) : this(list.first())
}

class Stamp(date: java.util.Date) {
  private constructor(date: java.sql.Date) : this(java.util.Date(date.time))
}
