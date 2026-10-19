package com.example.app

import kotlin.math.max as maximum

/** Doc with [Link] and @param x the value */
@Suppress("UNUSED") @get:JvmName("n")
class Box<T : Any>(val size: Int, private var name: String) : Base(), Comparable<Box<T>> {
  private val items = mutableListOf<T>()
  var count: Int = 0
    get() = field + 1
    set(value) { field = value }
  companion object { const val LIMIT = 10; val DEFAULT = Box<Any>(1, "a") }

  fun add(item: T, times: Int = 1): Boolean {
    count++
    var total = times
    total += 1
    items.add(item)
    val text = "x=$count, ${items.size} items\n"
    loop@ for (i in 0 until times) { if (i > LIMIT) break@loop }
    run(block = { it -> it.toString() })
    return items.isNotEmpty() && text.length > 0x1F
  }

  override fun compareTo(other: Box<T>): Int = size.compareTo(other.size)
}

enum class Color { RED, GREEN }
typealias Name = String
fun interface Action { fun run() }
