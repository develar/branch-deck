@file:JvmName("Util")
package com.example.facade

fun helper() = 1

const val LIMIT = 2

class Holder {
  @JvmField val field = 1
  @get:JvmName("customName") val renamed = 2
  val plain = 3
  @JvmName("javaName") fun kotlinName() = 4
  lateinit var late: String
  var mutable = 5
  var hidden = 6
    private set
  @JvmSynthetic fun synthetic() = 7
}

class Params(var a: Int, val b: Int)
