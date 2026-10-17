package com.example.refs

import com.example.shapes.Circle

/**
 * KDoc talks about [Circle] and Phantom and `Ghost`.
 */
class Renderer(private val canvas: Canvas) {
  // a line comment about Phantom
  /* a block comment about Ghost */
  fun render(shapes: List<Shape>, scale: Int = DEFAULT_SCALE) {
    val title = "rendering Phantom ${shapes.size} shapes of $scale"
    val (first, second) = shapes
    for ((index, shape) in shapes.withIndex()) {
      canvas.draw(shape, index)
    }
    shapes.forEach { canvas.outline(it) }
    shapes.map { s -> s.area }.filter { area -> area > threshold }
    try {
      canvas.flush(title)
    } catch (e: IOException) {
      log(e.message, level = Level.WARNING, tag = first)
    }
    when (val count = shapes.size) {
      0 -> println(count)
      else -> canvas.status(count = count, second = second)
    }
    canvas.layer { draw(this@Renderer) }
    val ref = ::render
    val length = String::length
    com.example.util.Logger.log("fq")
    shapes.first() to shapes.last()
  }

  private val threshold = 0.5
}
