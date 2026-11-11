#![allow(warnings)]
#[derive(Clone, Copy)] struct Var;
fn s() {}
fn f(_: fn()) -> bool { true } fn p(_: Var) -> bool { true } fn o() -> Option<Var> { Some(Var) }
fn m01() { let s = Var; p(s); }
fn m02() { f(s); let s = Var; p(s); }
fn m03() { let s = { f(s); Var }; p(s); }
fn m04() { if let Some(s) = o() { p(s); } else { f(s); } f(s); }
fn m05() { while let Some(s) = o() { p(s); break; } f(s); }
fn m06() { let Some(s) = o() else { f(s); return; }; p(s); }
fn m07() { match o() { Some(s) if p(s) => { p(s); } _ => { f(s); } } f(s); }
fn m08() { for s in [Var] { p(s); } f(s); }
fn m09() { let g = |s: Var| p(s); f(s); }
fn m11(s: Var) { p(s); let g = || p(s); }
fn m12() { { let s = Var; p(s); } f(s); }
fn m13() { if let Some(s) = o() && p(s) { p(s); } f(s); }
fn m14() { let (s, t) = (Var, 1); p(s); }
fn m15() { loop { let s = Var; p(s); break; } f(s); }
fn m17() { let x = if let Some(s) = o() { p(s) } else { f(s) }; }
fn m18() { let s = Var; println!("{}", p(s)); }
fn m19() { match o() { Some(s) => p(s), None => f(s) }; }
fn m20() { for s in [Var] { p(s); }; for t in [1] { f(s); } }
fn m21() { let g = |x: i32| { let s = Var; p(s) }; f(s); }
fn m22() { if let Some(s) = o() { p(s); } else if f(s) { f(s); } }
fn m23() { let _ = o().map(|s| p(s)); f(s); }
fn m24() { 'a: loop { let s = Var; p(s); break 'a; } f(s); }
fn m25() { let Some(s) = o() else { panic!() }; p(s); }
fn m26() { while let Some(s) = o() && p(s) { p(s); } f(s); }
fn m27() { let x = match o() { Some(s) if { f2(); p(s) } => 1, _ => 2 }; f(s); }
fn m30() { let s = Var; mod inner { fn g() { super::f(super::s); } } p(s); }
fn m31() { let t = (|| { let s = Var; p(s) })(); f(s); }
fn m32() { if f(s) { let s = Var; p(s); } else { f(s); } }
fn m33() { let s = Var; let s2 = async { p(s) }; }
fn m34() { let x = loop { let s = Var; if p(s) { break 1; } }; f(s); }
fn m36() { let v = [Var].iter().map(|&s| p(s)).count(); f(s); }
fn m37() { let s = Var; let g = |x: Var| { p(s); p(x) }; }
fn m38() { if let Some(s) = o() { p(s); } else { let s = Var; } f(s); }
fn m39() { fn inner() { f(s); } let s = Var; p(s); }
fn m40() { let t = { let s = Var; p(s) }; struct L; impl L { fn g() { f(s); } } }
fn f2() {}
