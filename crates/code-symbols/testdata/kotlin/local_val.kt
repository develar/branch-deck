package com.example.journey

class Journey {
  fun step(turns: List<Turn>) {
    val turnId = turns.first().id
    check(turnId > 0)
    turns.forEach { turn ->
      val turnId = turn.id
      record(turnId)
    }
    report(turnId)
  }
}

class Recorder {
  val turnId: Int = nextTurn()

  fun record() = store(turnId)
}
