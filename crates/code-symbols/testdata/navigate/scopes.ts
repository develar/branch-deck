class Fld { private a = 1 }
class Var { private b = 1 }
declare const s: Fld
declare function f(x: Fld): boolean
declare function p(x: Var): boolean
declare function o(): Var | undefined
function m01() { const s = new Var(); p(s) }
function m02() { { const s = new Var(); p(s) } f(s) }
function m03() { for (const s of [new Var()]) { p(s) } f(s) }
function m04() { for (let s = new Var(); p(s); p(s)) { p(s) } f(s) }
function m05() { try { f(s) } catch (s) { } finally { f(s) } }
function m06() { const g = (s: Var) => p(s); f(s) }
function m07() { const g = function (s: Var) { return p(s) }; f(s) }
function m08(s: Var) { p(s); const g = () => p(s) }
function m09() { if (f(s)) { var s2 = 1 } f(s) }
function m10(c: boolean) { if (c) { var s = new Var() } else { var s = new Var() } p(s) }
function m11() { class L { s = new Var(); g() { return f(s) } } }
function m12() { const { s } = { s: new Var() }; p(s) }
function m13() { const [s] = [new Var()]; p(s) }
function m14() { const g = ({ s }: { s: Var }) => p(s); f(s) }
function m15(k: number) { switch (k) { case 1: { const s = new Var(); p(s); break } default: f(s) } }
function m16(k: number) { switch (k) { case 1: let s = new Var(); p(s); break; case 2: s = new Var(); p(s) } f(s) }
function m17() { const g = function s(x: Var): boolean { return f2(s) || p(x) }; f(s) }
function m18() { label: for (const s of [new Var()]) { p(s); break label } f(s) }
function m19() { const t = [new Var()].map((s) => p(s)); f(s) }
function m20(x: Var = (f(s), new Var())) { const s = x; p(s) }
function m21(s: Var, t = p(s)) { p(s) }
function m22() { do { const s = new Var(); p(s) } while (f(s)) }
function m23() { while (f(s)) { const s = new Var(); p(s) } }
function m24() { for (const k in { a: 1 }) { const s = new Var(); p(s) } f(s) }
function m25() { const s = new Var(); function inner() { return p(s) } class L { g() { return p(s) } } }
function m26() { const g = (s: Var) => { let u: typeof s; return p(s) }; let t: typeof s }
function m27() { if (f(s)) { function s3() {} } f(s) }
function m28() { const t = { g(s: Var) { return p(s) }, s: f(s) } }
function m29() { enum E { s = 1, t = s } f(s) }
function m30() { const g = async (s: Var) => { await 1; return p(s) }; f(s) }
function m31() { const g = function* (s: Var) { yield p(s) }; f(s) }
function m32() { try { throw 1 } catch { f(s) } }
function m33() { for (const s of [new Var()]) p(s); f(s) }
function m34(c: boolean) { if (c) { const s = new Var(); p(s) } else { f(s) } f(s) }
function m35() { const t = ((s: Var) => p(s))(new Var()); f(s) }
function m36() { class L { constructor(private s: Var) { p(s) } g() { return f(s) } } }
function m37() { const g = (...s: Var[]) => p(s[0]); f(s) }
declare function f2(x: unknown): boolean
