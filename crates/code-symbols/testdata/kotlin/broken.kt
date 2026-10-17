package com.example.broken

class Valid {
  fun ok() = 1
}

class Broken {
  fun first() = 1
  @@@ ### $$$ %%% ^^^ &&& *** ((( )))
  fun second() = first()
}

fun interface Callback { fun call() }
