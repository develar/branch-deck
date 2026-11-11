FLD = "the name of the file"
VAR = "a name of a def"
s = FLD
def f(x): return x == FLD or fail("a name of a def read as the name of the file")
def p(x): return x == VAR or fail("the name of the file read as a name of a def")
def m01(): s = VAR; p(s)
def m02(xs = [VAR]): t = [p(s) for s in xs]; f(s)
def m03(): g = lambda s: p(s); g(VAR); f(s)
def m04(s): p(s)
def m05(s = VAR, t = f(s)): p(s)
def m06(): t = [p(s) for s in [VAR] if p(s)]; f(s)
def m07(): t = {k: p(s) for k, s in [(1, VAR)]}; f(s)
def m08(): t = [f(s) for x in [f(s)]]
def m09(): t = [p(s) for s in [VAR] for u in [p(s)]]; f(s)
def m10(): g = lambda x, t = f(s): x; g(1)
def m11(): g = lambda s = VAR: p(s); g(); f(s)
def m12(): s, t = VAR, 1; p(s)
def m13(): t = [[p(s) for u in [1]] for s in [VAR]]; f(s)
def m14(): g = lambda: f(s); g()
def m15(): s = VAR; g = lambda: p(s); g()
def m16(xs = [VAR]): t = [f(s) for x in xs]
def m17(): t = [p(s) for s in [VAR] if [p(s) for u in [1]]]
def m18(): t = [p(s) for x in [VAR] for s in [x]]; f(s)
def m19(): g = lambda: [p(s) for s in [VAR]]; g(); f(s)
def m20(): t = [f(s) for x in [1] for u in [f(s)]]
def m21(s = VAR): g = lambda t = p(s): p(s); g()
def m22(): t = (lambda s: p(s))(VAR); f(s)
def m23(): t = [x for x in [1] if f(s)]; g = lambda: f(s); g()
[m() for m in [m01, m02, m03, m05, m06, m07, m08, m09, m10, m11, m12, m13, m14, m15, m16, m17, m18, m19, m20, m21, m22, m23]]
m04(VAR)
