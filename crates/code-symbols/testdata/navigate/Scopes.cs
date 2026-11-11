using System.Linq;
class Fld {} class Var {} class Res : System.IDisposable { public void Dispose() {} }
class Scopes {
  static Fld s;
  static bool f(Fld x) => true; static bool p(Var x) => true; static bool Get(out Var v) { v = new Var(); return true; } static Res Use(out Var v) { v = new Var(); return new Res(); }
  void m01() { if (Get(out Var s)) { p(s); } else { p(s); } p(s); }
  void m02() { while (Get(out Var s)) { p(s); break; } f(s); }
  void m03() { do { } while (Get(out Var s) && p(s)); f(s); }
  void m04() { for (var i = Get(out Var s); p(s); p(s)) { p(s); } f(s); }
  void m05() { for (var i = 0; Get(out Var s) && i < 1; i++) { p(s); } f(s); }
  void m06() { foreach (var x in new[] { Get(out Var s) }) { p(s); } f(s); }
  void m07() { using (Use(out Var s)) { p(s); } f(s); }
  void m08(int k) { switch (Get(out Var s) ? k : 0) { case 1: p(s); break; } p(s); }
  void m09(int k) { switch (k) { case 1 when Get(out Var s): p(s); break; default: f(s); break; } f(s); }
  bool m10(int k) => k switch { 1 when Get(out Var s) => p(s), _ => f(s) };
  void m11() { System.Func<bool> g = () => Get(out Var s) && p(s); f(s); }
  void m12(bool c) { if (c) Get(out Var s); f(s); }
  void m13() { try { } catch (System.Exception) when (Get(out Var s) && p(s)) { p(s); } f(s); }
  bool m14() => Get(out Var s) && p(s);
  static bool fi = Get(out Var s) && p(s);
  static bool fj = f(s);
  bool Prop => Get(out Var s) && p(s);
  bool Prop2 { get => Get(out Var s) && p(s); }
  Scopes(int a) : this(Get(out Var s) && p(s)) { p(s); }
  Scopes(bool b) { f(s); }
  void m15() { var q = from x in new[] { 1 } let y = Get(out Var s) && p(s) select f(s); }
  void m16() { var q = from x in new[] { 1 } where Get(out Var s) && p(s) select f(s); }
  void m17() { bool L() => Get(out Var s) && p(s); f(s); }
  void m18() { { Get(out Var s); p(s); } f(s); }
  void m19() { lock (Use(out Var s)) { p(s); } p(s); }
  void m20() { lock (this) Get(out Var s); f(s); }
  void m21(bool c) { if (c) { } else Get(out Var s); f(s); }
  void m22() { foreach (var x in new[] { 1 }) Get(out Var s); f(s); }
  void m23() { while (true) { Get(out Var s); p(s); break; } f(s); }
  void m24() { using (var d = Use(out Var s)) p(s); f(s); }
  void m25() { var t = Get(out Var s) ? p(s) : p(s); p(s); }
  void m26() { Get(out Var s); System.Action a = () => p(s); a(); }
  void m27() { goto L; L: Get(out Var s); p(s); }
  void m28(int k) { switch (k) { case 1: Get(out Var s); p(s); break; case 2: Get(out s); p(s); break; } f(s); }
  void m29() { for (var i = 0; i < 1; Get(out Var s)) { f(s); } f(s); }
  void m30() { System.Func<int, bool> g = x => { Get(out Var s); return p(s); }; f(s); }
  bool m31() { return Get(out Var s) && p(s); }
  void m32() { var q = from x in new[] { 1 } from z in new[] { Get(out Var s) && p(s) } select f(s); }
  void m33() { var q = from x in new[] { Get(out Var s) } select p(s); p(s); }
  void m34() { var q = from x in new[] { 1 } join z in new[] { Get(out Var s) } on x equals 1 select p(s); p(s); }
  void m35() { var q = from x in new[] { 1 } orderby Get(out Var s) && p(s) select f(s); }
  void m36() { var q = from x in new[] { 1 } group Get(out Var s) && p(s) by f(s); }
  void m37() { var q = from x in new[] { 1 } select Get(out Var s) && p(s); f(s); }
  void m38(bool c) { while (c) Get(out Var s); f(s); }
  void m39() { using (Use(out Var s)) Get(out Var t); f(s); }
  void m40(object o) { if (o is Var s) { p(s); } f(Scopes.s); }
  void m41(int k) { switch (k) { case 1: { Get(out Var s); p(s); break; } default: f(s); break; } }
  void m42() { checked { Get(out Var s); p(s); } f(s); }
  void m43() { try { Get(out Var s); p(s); } finally { f(s); } }
  void m44() { for (Get(out Var s); p(s); ) { p(s); break; } f(s); }
  void m45() { do Get(out Var s); while (f(s)); f(s); }
  void n01() { foreach (Var s in new Var[0]) { p(s); } f(s); }
  void n02() { System.Func<Var, bool> g = (Var s) => p(s); f(s); }
  void n03() { bool L(Var s) => p(s); f(s); L(null); }
  void n04(object o) { if (o is Var s && p(s)) { p(s); } }
  void n05(object o) { switch (o) { case Var s: p(s); break; default: f(s); break; } f(s); }
  bool n06(object o) => o switch { Var s when p(s) => p(s), _ => f(s) };
  void n07() { for (Var s = null; p(s); p(s)) { p(s); } f(s); }
  void n08() { { Var s = null; p(s); } f(s); }
  void n09(Var s) { p(s); System.Action a = () => p(s); }
  void n10() { var q = from Var s in new Var[0] where p(s) select p(s); f(s); }
  void n11() { var q = from x in new[] { 1 } let y = f(s) select y; }
  void n12(object o) { while (o is Var s) { p(s); o = null; } f(s); }
  void n13(object o) { if (!(o is Var s)) { return; } p(s); }
  void n14(int k) { switch (k) { case 1: Var s = null; p(s); break; case 2: s = null; p(s); break; } f(s); }
  void n15() { Var s = null; { p(s); } System.Action a = () => { p(s); }; }
  void n16() { var q = from x in new Var[0] group x by x into g from Var s in g where p(s) select p(s); f(s); }
  void n17() { var q = from x in new Var[0] join Var s in new Var[0] on f(s) equals p(s) select p(s); f(s); }
  void n18() { using (Res r = new Res()) { f(s); } f(s); }
  void n19() { try { Var s = null; p(s); } catch (System.Exception) { f(s); } finally { f(s); } }
  void n20(object o) { var t = o is Var s ? p(s) : f(Scopes.s); }
  static void Main() { }
}
