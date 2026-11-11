class Scopes {
  static class Field {} static class Pat {} static class Res implements AutoCloseable { public void close() {} }
  Field s;
  boolean f(Field x) { return true; } boolean p(Pat x) { return true; } Res r(Res x) { return x; }
  void m01(Object o) { if (o instanceof Pat s) { p(s); } else { f(s); } f(s); }
  void m02(Object o) { if (!(o instanceof Pat s)) { f(s); return; } p(s); }
  void m03(Object o) { if (!(o instanceof Pat s)) { f(s); } else { p(s); } f(s); }
  boolean m04(Object o) { return o instanceof Pat s && p(s) || f(s); }
  boolean m05(Object o) { return !(o instanceof Pat s) || p(s); }
  boolean m06(Object o) { return o instanceof Pat s ? p(s) : f(s); }
  boolean m07(Object o) { return !(o instanceof Pat s) ? f(s) : p(s); }
  void m08(Object o) { while (o instanceof Pat s) { p(s); o = null; } f(s); }
  void m09(Object o) { while (!(o instanceof Pat s)) { f(s); o = new Pat(); } p(s); }
  void m10(Object o) { while (!(o instanceof Pat s)) { f(s); break; } f(s); }
  void m11(Object o) { do { f(s); o = new Pat(); } while (!(o instanceof Pat s)); p(s); }
  void m12(Object o) { for (; o instanceof Pat s; p(s)) { p(s); } f(s); }
  void m13(Object o) { for (; !(o instanceof Pat s); f(s)) { f(s); o = new Pat(); } p(s); }
  void m14(Object o) { outer: while (!(o instanceof Pat s)) { for (;;) { break outer; } } f(s); }
  void m15(Object o) { x: { while (!(o instanceof Pat s)) { if (o == null) break x; o = new Pat(); } p(s); } f(s); }
  void m16(Object o) { l: while (!(o instanceof Pat s)) { o = new Pat(); } p(s); }
  void m17(Object o) { switch (o) { case Integer n when o instanceof Pat s: p(s); break; default: f(s); } }
  void m18(Object o) { switch (o) { case Integer n when !(o instanceof Pat s) -> f(s); default -> f(s); } }
  void m19(Object o) { switch (o) { case Integer n when o instanceof Pat s -> p(s); default -> f(s); } }
  void m20(int k, Object o) { switch (k) { case 1: if (!(o instanceof Pat s)) return; p(s); default: f(s); } }
  void m21(Object o) { if (!(o instanceof Pat s)) { synchronized (this) { return; } } p(s); }
  void m22(Object o) { if (!(o instanceof Pat s)) { while (true) { } } p(s); }
  void m23(Object o) { if (!(o instanceof Pat s)) { try { return; } finally { } } p(s); }
  void m24(Object o) { if (!(o instanceof Pat s)) { try { } finally { return; } } p(s); }
  void m25(Object o) { if (!(o instanceof Pat s)) { try { return; } catch (RuntimeException e) { } } f(s); }
  void m26(Object o) { if (!(o instanceof Pat s)) { l: { return; } } p(s); }
  void m27(Object o) { if (!(o instanceof Pat s)) { l: { if (o == null) break l; return; } } f(s); }
  void m28(int k, Object o) { if (!(o instanceof Pat s)) { switch (k) { default: return; } } p(s); }
  void m29(int k, Object o) { if (!(o instanceof Pat s)) { switch (k) { case 1: return; } } f(s); }
  void m30(int k, Object o) { if (!(o instanceof Pat s)) { switch (k) { case 1 -> { return; } default -> throw new RuntimeException(); } } p(s); }
  void m31(int k, Object o) { if (!(o instanceof Pat s)) { switch (k) { case 1 -> f(s); default -> throw new RuntimeException(); } } f(s); }
  void m32(Object o) { if (!(o instanceof Pat s)) { for (;;) { } } p(s); }
  void m33(Object o) { if (!(o instanceof Pat s)) { do { } while (true); } p(s); }
  void m34(Object o) { if (!(o instanceof Pat s)) { do { if (o == null) continue; return; } while (o != null); } f(s); }
  void m35(Object o) { if (!(o instanceof Pat s)) { if (o == null) return; else throw new RuntimeException(); } p(s); }
  void m36(Object o) { if (o instanceof Pat s) { p(s); } else if (o != null) { f(s); } f(s); }
  void m37(Object o) { if (o instanceof Pat s) { p(s); } else { return; } p(s); }
  void m38(Object o) { boolean b = o instanceof Pat s && p(s); f(s); }
  void m39(Object o) { if (o instanceof Pat s == true) { f(s); } }
  void m40(Object o) { if ((o instanceof Pat s) && o != null) { p(s); } }
  void m41(Object o) { if (o != null && !(o instanceof Pat s)) { f(s); return; } f(s); }
  void m42(Object o) { if (o == null || !(o instanceof Pat s)) { f(s); return; } p(s); }
  void m43(Object o) { if (!(o instanceof Pat s)) throw new IllegalStateException(); p(s); }
  void m44(Object o) { for (int i = 0; i < 1; i++) { if (!(o instanceof Pat s)) continue; p(s); } f(s); }
  void m45(Object o) { Runnable run = () -> { if (!(o instanceof Pat s)) return; p(s); }; f(s); }
  void m46(Object o) { if (!(o instanceof Pat s)) { while (true) { break; } } f(s); }
  void m47(Object o) { if (!(o instanceof Pat s)) { while (true) { for (;;) { break; } } } p(s); }
  void m48(Object o) { if (!(o instanceof Pat s)) { while (true) { Runnable run = () -> { while (true) { break; } }; } } p(s); }
  void m49(Object o) { if (!(o instanceof Pat s)) { while (o != null) { } return; } p(s); }
  void m50(Object o) { if (!(o instanceof Pat s)) { for (int i = 0; ; i++) { } } p(s); }
  void m51(Object o) { if (!(o instanceof Pat s)) { for (int i = 0; i < 2; i++) { } } f(s); }
  void m52(Object o) { use(o instanceof Pat s, f(s)); }
  void m53(Object o) { if (!(o instanceof Pat s)) { switch (o) { case Integer n -> { return; } case Object x -> { return; } } } p(s); }
  void m54() throws Exception { try (Res s = new Res()) { r(s); } finally { f(s); } }
  void m55() throws Exception { try (Res s = new Res(); Res q = r(s)) { r(s); } catch (Exception e) { f(s); } f(s); }
  void m56(Object o) { if (o instanceof Pat s && p(s) && p(s)) { p(s); } }
  void m57(Object o) { if (!(o instanceof Pat s) || !p(s)) { f(s); return; } p(s); }
  void n01() { { Pat s = null; p(s); } f(s); }
  void n02() { for (Pat s = null; p(s); p(s)) { p(s); } f(s); }
  void n03(java.util.List<Pat> xs) { for (Pat s : xs) { p(s); } f(s); }
  void n04() { java.util.function.Predicate<Pat> g = s -> p(s); f(s); }
  void n05() { try { f(s); } catch (RuntimeException e) { f(s); } }
  void n06(int k) { switch (k) { case 1: Pat s = null; p(s); break; case 2: s = null; p(s); break; } f(s); }
  void n07(int k) { switch (k) { case 1 -> { Pat s = null; p(s); } default -> f(s); } }
  void n08(Pat s) { p(s); Runnable run = () -> p(s); }
  void n09() { f(s); Pat s = null; p(s); }
  void n10() { Pat s = null; class L { boolean g() { return p(s); } } }
  void n11() { Object o = new Object() { boolean g() { return f(s); } }; }
  void n12(Object o) { switch (o) { case Pat s -> p(s); default -> f(s); } }
  void n13(Object o) { switch (o) { case Pat s when p(s): p(s); break; default: f(s); } f(s); }
  boolean n14(Object o) { return switch (o) { case Pat s -> p(s); default -> f(s); }; }
  void n15() { class L { Pat s; boolean g() { return p(s); } } f(s); }
  void n16() { do { Pat s = null; p(s); } while (f(s)); }
  void n17() { if (f(s)) { Pat s = null; p(s); } else f(s); }
  void n18() { Runnable run = () -> { Pat s = null; p(s); }; f(s); }
  void n19() throws Exception { try (Res q = null) { Pat s = null; p(s); } catch (RuntimeException e) { f(s); } }
  void n20(java.util.List<Pat> xs) { for (Pat s : xs) p(s); f(s); }
  class Inner { boolean g() { return f(s); } }
  void use(boolean a, boolean b) {}
}
