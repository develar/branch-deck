class Fld
class Var
val s = Fld()
fun f(x: Fld) = true
fun p(x: Var) = true
fun m01() { val s = Var(); p(s) }
fun m02() { val s = Var().also { f(s) }; p(s) }
fun m03() { for (s in listOf(Var())) { p(s) }; f(s) }
fun m04() { do { val s = Var() } while (p(s)); f(s) }
fun m05() { when (val s = Var()) { else -> p(s) }; f(s) }
fun m06() { try { val s = Var(); p(s) } catch (e: Exception) { f(s) } finally { f(s) } }
fun m07() { listOf(Var()).forEach { s -> p(s) }; f(s) }
fun m08() { val g = fun(s: Var): Boolean { return p(s) }; f(s) }
fun m09(c: Boolean) { if (c) { val s = Var(); p(s) } else { f(s) }; f(s) }
fun m10() { val (s, t) = Pair(Var(), 1); p(s) }
fun m11(s: Var) { p(s); val g = { p(s) } }
fun m12() { val o = object { val s = Var(); fun g() = p(s) }; f(s) }
fun m13() { val s = Var(); val o = object { val x = Fld(); fun g() = p(s) } }
fun m14() { fun inner(s: Var): Boolean { return p(s) }; f(s) }
fun m15() { while (true) { val s = Var(); p(s); break; }; f(s) }
fun m16() { val t = run { val s = Var(); p(s) }; f(s) }
class K1(s: Var) { val a = p(s); fun g() = f(s); init { p(s) } }
class K2(val s: Var) { fun g() = p(s) }
class K3 { val s = Var(); fun g() = p(s) }
fun m17() { val g = { s: Var -> p(s) }; f(s) }
fun m18() { for ((s, t) in listOf(Pair(Var(), 1))) { p(s) }; f(s) }
fun m19() { val l = listOf(Var()).map { s -> { p(s) } }; f(s) }
fun m20() { val s = Var(); fun inner(): Boolean { return p(s) }; class L { fun g() = p(s) } }
fun m21() { class L(s: Var) { val a = p(s); fun g() = f(s) } }
fun m22() { val x = when { else -> { val s = Var(); p(s) } }; f(s) }
fun m23() { val g: (Var) -> Boolean = { s -> p(s) }; f(s) }
fun m24(c: Boolean) { if (c) { val s = Var(); p(s) }; f(s) }
fun m25() { val s = Var(); val g = { x: Int -> p(s) }; val o = object : Runnable { override fun run() { p(s) } } }
fun m26(v: Var?) { v?.let { s -> p(s) }; f(s) }
fun m27() { f(s); val s = Var(); p(s) }
fun m28() { val g = { f(s); val s = Var(); p(s) } }
class K4 { fun g() { p(s) }; companion object { val s = Var(); fun h() = p(s) } }
class K5 { companion object { val s = Var() }; fun g() = p(s) }
object O1 { val s = Var(); fun g() = p(s) }
enum class E1(val s: Var) { A(Var()); fun g() = p(s) }
class K6 { val s = Var(); inner class I { fun g() = p(s) }; class N { fun g() = f(s) } }
interface I1 { val s: Var; fun g() = p(s) }
fun Var.ext() { val g = { p(this) }; f(s) }
fun m29() { val s by lazy { Var() }; p(s) }
fun m30() { var s = Var(); s = Var(); p(s) }
fun m31() { for (s in listOf(Var())) p(s); f(s) }
fun m32(c: Boolean) { while (c) { val s = Var(); p(s) }; f(s) }
