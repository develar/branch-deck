package com.example.delegation

import com.example.base.Base

class Primary(val size: Int, val name: String = "") {
  constructor(name: String) : this(name.length, name)

  constructor() : this(size = 0)
}

class OnlySecondary : Base {
  constructor(size: Int) : super(size, "secondary")

  constructor() : this(0)
}

class Qualified : com.example.base.Base {
  constructor(size: Int) : super(size)
}

class TwoSupertypes : Base, Runnable {
  constructor(size: Int) : super(size)

  override fun run() {}
}

class NoSupertype {
  constructor(size: Int) : super()
}

open class Outer {
  class Nested(val size: Int) {
    constructor() : this(1)
  }
}

enum class Level(val code: Int) {
  LOW(1);

  constructor() : this(0)
}

fun local() {
  class InFunction(size: Int) {
    constructor() : this(0)
  }
}
