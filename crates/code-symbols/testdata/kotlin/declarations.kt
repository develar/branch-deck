@file:JvmName("Declarations")
package com.example.shapes

/** A shape. */
interface Shape {
  val area: Double
  fun describe(): String
}

internal class Circle(val radius: Double, private val label: String, scale: Int) : Shape {
  override val area: Double = Math.PI * radius * radius * scale

  override fun describe(): String = "circle $label"

  private fun helper() = Unit

  inner class Inner
  class Nested

  companion object {
    const val UNIT = 1.0
    fun unit(): Circle = Circle(UNIT, "unit", 1)
  }

  var counter: Int = 0
    get() = field
    set(value) {
      field = value
    }
}

object Registry : Shape by Circle(1.0, "registry", 1)

object Shapes2 : Shape {
  fun register(shape: Shape) = shape
}

object Tiny { val size = 1 }

enum class Color(val rgb: Int) {
  RED(0xFF0000),
  GREEN(0x00FF00) {
    override fun toString() = "green"
  };

  fun hex(): String = rgb.toString(16)

  companion object Factory {
    fun of(rgb: Int) = entries.first { it.rgb == rgb }
  }
}

sealed interface Result<out T>
data class Ok<T>(val value: T) : Result<T>
annotation class Marker

typealias Shapes = List<Shape>

private fun topSecret() = 42
internal val moduleWide = "x"
val String.shout: String get() = uppercase()
fun Shape?.orEmpty(): Shape = this ?: Registry
fun <T : Shape> List<T>.largest(): T? = maxByOrNull { it.area }
expect fun platformName(): String
