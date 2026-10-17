package com.example.misread

/** Two modifiers before `class` or `interface`, and any declaration after it, make the grammar read infix calls. */
internal sealed interface Registrar {
  val state: Int
  fun register(id: String)
}

internal abstract class Base {
  abstract fun name(): String
}

object Tiny { val size = 1 }

@Suppress("unused")
@Deprecated("hidden")
private sealed class Hidden {
  private companion object {
    const val LIMIT = 1
  }

  fun limit() = LIMIT
}

class After(val registrar: Registrar) {
  fun run() = registrar.register("after")
}
