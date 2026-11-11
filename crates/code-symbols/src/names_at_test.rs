//! The names of many places from one parse of a text (`navigate::names_at`,
//! openspec/specs/commit/name-bindings/spec.md): a table per language of what a name is bound to, what a body declares
//! it as, what each site makes of it and what another name for it would clash with

use crate::highlight::Grammar;
use crate::navigate::{Access, ClashKind, LocalKind, NameAt, NameQuestion, Position, Site, names_at, usages_at};
use crate::test_support::place;
use pretty_assertions::assert_eq;

/// An answer as text: what declares the local (`-` for any other name), its uses as `line:column access site`, then
/// after `|` the clashes of the new name asked, `no clash` where it has none
fn describe(found: &NameAt) -> String {
  let local = found.local.map_or_else(
    || "-".to_string(),
    |it| {
      let kind = match it.kind {
        LocalKind::Variable => "variable",
        LocalKind::Parameter => "parameter",
        LocalKind::LambdaParameter => "lambda parameter",
        LocalKind::LoopVariable => "loop variable",
        LocalKind::CatchParameter => "catch parameter",
        LocalKind::Other => "other",
      };
      if it.named_by_calls { format!("{kind} named by calls") } else { kind.to_string() }
    },
  );
  let uses: Vec<String> = found
    .uses
    .iter()
    .map(|it| {
      let site = match it.site {
        Site::Plain => "",
        Site::Shorthand => " shorthand",
        Site::NamedArgument => " named argument",
        Site::Label => " label",
        Site::Namesake => " namesake",
        Site::Spelled => " spelled",
        Site::Macro => " macro",
        Site::Destructured => " destructured",
      };
      format!("{}:{} {}{site}", it.start.line, it.start.column, it.access.name())
    })
    .collect();
  let clashes = found.clashes.as_ref().map_or_else(String::new, |clashes| {
    let clashes: Vec<String> = clashes
      .iter()
      .map(|it| {
        let kind = match it.kind {
          ClashKind::Shadowed => "shadowed",
          ClashKind::Captured => "captured",
          ClashKind::Implicit => "implicit",
        };
        format!("{}:{} {kind}", it.start.line, it.start.column)
      })
      .collect();
    format!(" | {}", if clashes.is_empty() { "no clash".to_string() } else { clashes.join(", ") })
  });
  format!("{local}: {}{clashes}", uses.join(", "))
}

/// The answer for the `occurrence`-th `needle` of a text, asked with a new name or none
fn ask(grammar: Grammar, text: &str, needle: &str, occurrence: usize, renamed_to: Option<&str>) -> String {
  let question = NameQuestion {
    position: place(text, needle, occurrence),
    renamed_to,
  };
  names_at(grammar, text, &[question]).of(0).map_or_else(|| "none".to_string(), describe)
}

/// Each row of a table: the `occurrence`-th `needle` of the text, a new name for it or none, and its answer
fn check(grammar: Grammar, text: &str, rows: &[(&str, usize, Option<&str>, &str)]) {
  let found: Vec<_> = rows
    .iter()
    .map(|&(needle, occurrence, renamed_to, _)| (needle, occurrence, renamed_to, ask(grammar, text, needle, occurrence, renamed_to)))
    .collect();
  let expected: Vec<_> = rows
    .iter()
    .map(|&(needle, occurrence, renamed_to, answer)| (needle, occurrence, renamed_to, answer.to_string()))
    .collect();
  assert_eq!(found, expected, "in\n{text}");
}

const KOTLIN: &str = r#"class Ledger(val total: Long) {
  fun balance(entries: List<Entry>, scale: Int): Long {
    var total = 0L
    for (entry in entries) {
      total += entry.amount * scale
      run { val scale = 2; total += scale }
    }
    val (first, second) = pair(total)
    report(total = total, sum = this.total)
    try { total() } catch (e: Exception) { log(e, ::total, "$total ${first}") }
    entries.forEach { item -> use(item, it, sum) }
    return total
  }
  val sum = 1
}
"#;
const KOTLIN_KINDS: &str = "class Box {
  var size = 0
    set(value) { field = value }
  fun open(handler: (event: Event) -> Unit) {
    val callback = fun(code: Int) = code + 1
    when (val state = load()) { is Ready -> use(state) }
    class Local(val width: Int, depth: Int) { val area = width * depth }
    fun inner(count: Int) = count
    val label: handler = handler
    for ((key, value) in pairs) { use(key, value) }
    pairs.map { (left, right) -> left + right }
  }
}
";
const JAVA: &str = "class Ledger {
  long total;
  long sum() { return 0; }
  long balance(List<Entry> entries, int scale) {
    long total = 0;
    for (Entry entry : entries) {
      total += entry.amount * scale;
      { int inner = 2; total += inner; }
    }
    for (int i = 0; i < scale; i++) { total++; }
    outer: for (;;) { break outer; }
    try (Reader in = open()) { total(); } catch (Exception e) { log(e, this.total, sum); }
    entries.forEach(item -> use(item, sum()));
    if (first instanceof Point(int x, int y) && other instanceof String text) { use(x, text); }
    @Range(scale = 1) int ranged = total;
    total local = null;
    return total;
  }
}
";
const CSHARP: &str = r#"class Ledger {
  long total;
  long Balance(List<Entry> entries, int scale) {
    long total = 0;
    foreach (var entry in entries) {
      total += entry.Amount * scale;
      { int inner = 2; total += inner; }
    }
    for (var i = 0; i < scale; i++) { total++; }
    var pair = (total, scale);
    var made = new { total, Name = scale };
    var box = new Box { total = total };
    Report(total: total, sum: this.total);
    var text = nameof(total) + $"{total}";
    var (first, second) = pair;
    try { Use(out var seen); } catch (Exception e) { Log(e, sum); }
    Func<int, int> twice = item => item * 2;
    int Local(int size) => size + total;
    total other = default;
    return total;
  }
  long sum;
  long Size { set { var size = value; } }
}
"#;
const STARLARK: &str = "total = 1

def _library(name, deps = [], *args, **kwargs):
    total = len(deps)
    for dep in deps:
        total += size(dep, scale = total)
    first, second = pair(total)
    names = [name for name in deps if name]
    key = lambda item: item + total
    native.cc_library(name = name, deps = deps, total = total, size = size)
    def inner(total):
        return total
    return struct(total = total, args = args)

size = 2
";
const RUST: &str = r#"struct Ledger { total: u64 }

fn balance(entries: &[Entry], scale: u64) -> u64 {
  let mut total = 0;
  for entry in entries {
    total += entry.amount * scale;
    { let scale = 2; total += scale; }
  }
  let ledger = Ledger { total };
  let other = Ledger { total: total };
  let Ledger { total: read } = ledger;
  let Ledger { total } = other;
  let (first, second) = pair(total);
  let twice = |item: u64| item * sum;
  println!("{} {read} {{read}} {:first$}", read, 1);
  read!(first);
  let typed: read = first;
  if let Some(found) = find(read) { use_it(found); }
  match second { 0 => sum, more => more }
}

const sum: u64 = 1;
"#;
const TYPESCRIPT: &str = "class Ledger {
  total = 0
  constructor(private scale: number, plain: number) { use(plain) }
  balance(entries: Entry[], scale: number): number {
    let total = 0
    for (const entry of entries) {
      total += entry.amount * scale
      { const scale = 2; total += scale }
    }
    for (let i = 0; i < scale; i++) total++
    const made = { total, sum: this.total }
    const { first, second: renamed, ...rest } = made
    const [head] = entries
    ;({ total } = made)
    ;[total] = [1]
    try { total() } catch (error) { log(error, sum) }
    entries.forEach((item) => use(item, sum))
    const typed: total = `${total}`
    function inner({ size }: Box, [width]: number[]) { return size + width }
    return total
  }
}
const sum = 1
type total = string
";
const TSX: &str = "export function Ledger({ entries, scale }: Props) {
  const [total, setTotal] = useState(0)
  const rows = entries.map((entry) => <Row key={entry.id} total={total} entry={entry} {...{ scale }} />)
  return <Total total={total}>{rows}{sum}</Total>
}
const sum = 1
";
const JAVASCRIPT: &str = "function balance(entries, scale) {
  var total = 0
  for (const entry of entries) {
    total += entry.amount * scale
    { let scale = 2; total += scale }
  }
  const { first, second: renamed } = pair(total)
  const made = { total, sum: this.total }
  try { total() } catch (error) { log(error, sum) }
  return function inner(scale) { return scale + total }
}
var sum = 1
";
#[test]
fn kotlin_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Kotlin,
    KOTLIN,
    &[
      // a local with several uses: a named argument, a call, a callable reference and a string template among them; `sum` is a property its body reads
      (
        "total = 0L",
        0,
        Some("sum"),
        "variable: 2:8 declaration, 4:6 write, 5:27 write, 7:31 read, 8:11 read named argument, 8:19 read, 9:10 call namesake, 9:52 read namesake, 9:61 read, 11:11 read | 10:44 captured",
      ),
      // the property of the same spelling: no use of the local is its
      ("total: Long", 0, None, "-: 0:17 declaration, 8:37 read"),
      // a parameter a call may name; the name of the loop variable is nearer to its use in the loop
      ("scale: Int", 0, Some("entry"), "parameter named by calls: 1:36 declaration, 4:30 read | 4:30 shadowed"),
      // the local of a nested block shadows the parameter and would take the write beside it
      ("scale = 2", 0, Some("total"), "variable: 5:16 declaration, 5:36 read | 5:27 captured"),
      // an entry of a destructuring
      ("first", 0, None, "variable: 7:9 declaration destructured, 9:69 read"),
      // a loop variable; `it` is bound without a declaration
      ("entry in", 0, Some("it"), "loop variable: 3:9 declaration, 4:15 read | 3:9 implicit"),
      // the parameter of a catch
      ("e: Exception", 0, None, "catch parameter: 9:27 declaration, 9:47 read"),
      // the parameter of a lambda
      ("item ->", 0, Some("sum"), "lambda parameter: 10:22 declaration, 10:34 read | 10:44 captured"),
    ],
  );
}

#[test]
fn kotlin_tells_what_a_local_is() {
  check(
    Grammar::Kotlin,
    KOTLIN_KINDS,
    &[
      // the parameter of a setter: no call names it
      ("value)", 0, None, "parameter: 2:8 declaration, 2:25 read"),
      // the parameter of a function type names nothing
      ("event:", 0, None, "other: 3:21 declaration"),
      // the parameter of a function without a name
      ("code:", 0, None, "lambda parameter: 4:23 declaration, 4:36 read"),
      // the subject of a when: its own value is read before it
      ("state =", 0, Some("load"), "variable: 5:14 declaration, 5:48 read | no clash"),
      // a val of the constructor of a local class is a member
      ("width:", 0, None, "other: 6:20 declaration, 6:57 read"),
      // a parameter of the constructor of a local class
      ("depth:", 0, None, "parameter named by calls: 6:32 declaration, 6:65 read"),
      // a property of a local class
      ("area", 0, None, "other: 6:50 declaration"),
      // the parameter of a local function
      ("count:", 0, None, "parameter named by calls: 7:14 declaration, 7:28 read"),
      // a local function
      ("inner", 0, None, "other: 7:8 declaration"),
      // a parameter read as a type is a type of that spelling
      ("handler:", 0, None, "parameter named by calls: 3:11 declaration, 8:15 read namesake, 8:25 read"),
      // an entry of the destructuring of a loop
      ("key,", 0, None, "loop variable: 9:10 declaration destructured, 9:38 read"),
      // an entry of the destructuring of a lambda
      ("left,", 0, None, "lambda parameter: 10:17 declaration destructured, 10:33 read"),
    ],
  );
}

#[test]
fn java_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Java,
    JAVA,
    &[
      // a local with several uses; a call and a type of its spelling are no value; `sum` is read as a value and called
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 4:9 declaration, 6:6 write, 7:23 write, 9:38 write, 11:31 call namesake, 14:35 read, 15:4 read namesake, 16:11 read | 11:83 captured, 12:38 captured",
      ),
      // the field of the same spelling
      ("total;", 0, None, "-: 1:7 declaration, 11:76 read"),
      // a parameter: Java has no named arguments
      ("scale)", 0, Some("entry"), "parameter: 3:40 declaration, 6:30 read, 9:24 read | 6:30 shadowed"),
      // the local of a nested block would take the write beside it
      ("inner = 2", 0, Some("total"), "variable: 7:12 declaration, 7:32 read | 7:23 captured"),
      // the variable of a for over a collection
      ("entry :", 0, None, "loop variable: 5:15 declaration, 6:15 read"),
      // the variable of a for with a head
      ("i = 0", 0, None, "loop variable: 9:13 declaration, 9:20 read, 9:31 write"),
      // a label and its jump: no scope binds them
      ("outer:", 0, Some("total"), "other: 10:4 declaration label, 10:28 read label | no clash"),
      // a resource
      ("in =", 0, None, "variable: 11:16 declaration"),
      // the parameter of a catch
      ("e)", 1, None, "catch parameter: 11:59 declaration, 11:68 read"),
      // the parameter of a lambda
      ("item ->", 0, Some("entries"), "lambda parameter: 12:20 declaration, 12:32 read | no clash"),
      // a variable of a record pattern
      ("x,", 0, None, "variable: 13:35 declaration destructured, 13:84 read"),
      // a pattern variable
      ("text)", 0, None, "variable: 13:72 declaration, 13:87 read"),
      // the element of an annotation
      ("scale =", 0, None, "-: 14:11 read named argument"),
    ],
  );
}

#[test]
fn csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::CSharp,
    CSHARP,
    &[
      // a local with several uses: an element of a tuple and a member of an anonymous type it names, a member an initializer names, `nameof`, an interpolation, a type
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 3:9 declaration, 5:6 write, 6:23 write, 8:38 write, 9:16 read shorthand, 10:21 read shorthand, 11:24 write namesake, 11:32 read, 12:18 read, 13:22 read spelled, 13:34 read, 17:34 read, 18:4 read namesake, 19:11 read | 15:60 captured",
      ),
      // the field of the same spelling, and the named argument
      ("total;", 0, None, "-: 1:7 declaration, 12:11 read named argument, 12:35 read"),
      // a parameter a call may name; the foreach variable is nearer in its loop only
      (
        "scale)",
        0,
        Some("entry"),
        "parameter named by calls: 2:40 declaration, 5:30 read, 8:24 read, 9:23 read shorthand, 10:35 read shorthand | 5:30 shadowed",
      ),
      // the local of a nested block
      ("inner = 2", 0, Some("total"), "variable: 6:12 declaration, 6:32 read | 6:23 captured"),
      // the variable of a foreach
      ("entry in", 0, None, "loop variable: 4:17 declaration, 5:15 read"),
      // the variable of a for
      ("i = 0", 0, None, "loop variable: 8:13 declaration, 8:20 read, 8:31 write"),
      // a variable of a deconstruction
      ("first", 0, None, "variable: 14:9 declaration destructured"),
      // an out variable
      ("seen", 0, None, "variable: 15:22 declaration"),
      // the variable of a catch
      ("e) { Log", 0, None, "catch parameter: 15:48 declaration, 15:57 read"),
      // the parameter of a lambda
      ("item =>", 0, None, "lambda parameter: 16:27 declaration, 16:35 read"),
      // the parameter of a local function
      ("size)", 0, None, "parameter named by calls: 17:18 declaration, 17:27 read"),
      // a local of a setter; `value` is bound without a declaration
      ("size =", 0, Some("value"), "variable: 22:24 declaration | 22:24 implicit, 22:31 captured"),
      // a local function
      ("Local", 0, None, "other: 17:8 declaration"),
    ],
  );
}

#[test]
fn starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Starlark,
    STARLARK,
    &[
      // a local with several uses, the names of named arguments among them; `size` is a name of the file its body calls
      (
        "total = len",
        0,
        Some("size"),
        "variable: 3:4 declaration, 5:8 read, 5:35 read, 6:25 read, 8:30 read, 9:48 read named argument, 9:56 read, 12:18 read named argument, 12:26 read | 5:17 captured, 9:70 captured",
      ),
      // the name of the file of the same spelling
      ("total", 0, None, "-: 0:0 declaration"),
      // a parameter a call may name, apart from the variable of the comprehension
      ("name,", 0, None, "parameter named by calls: 2:13 declaration, 9:22 read named argument, 9:29 read"),
      // a parameter; the loop variable is declared before its other uses
      (
        "deps =",
        0,
        Some("dep"),
        "parameter named by calls: 2:19 declaration, 3:16 read, 4:15 read, 7:30 read, 9:35 read named argument, 9:42 read | 4:15 shadowed, 7:30 shadowed, 9:42 shadowed",
      ),
      // the variable of a for
      ("dep in", 0, None, "loop variable: 4:8 declaration, 5:22 read"),
      // a name a tuple of targets assigns
      ("first", 0, None, "variable: 6:4 declaration destructured"),
      // the parameter of a lambda
      ("item:", 0, None, "lambda parameter: 8:17 declaration, 8:23 read"),
      // no call names what `*args` takes
      ("args", 0, None, "parameter: 2:31 declaration, 12:33 read named argument, 12:40 read"),
      // a def inside a def
      ("inner", 0, None, "other: 10:8 declaration"),
      // the parameter of the inner def shadows the local
      ("total):", 0, None, "parameter named by calls: 10:14 declaration, 11:15 read"),
      // the variable of a comprehension, seen in all of it
      ("name for", 0, None, "loop variable: 7:13 read, 7:22 declaration, 7:38 read"),
    ],
  );
}

#[test]
fn rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Rust,
    RUST,
    &[
      // a local with several uses: the field it names by its value, the name of a field; the binding of line 11 takes the later ones; `sum` is a constant its body reads
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 3:10 declaration, 5:4 write, 6:21 write, 8:24 read shorthand, 9:23 read namesake, 9:30 read, 10:15 read namesake | 13:33 captured, 18:22 captured",
      ),
      // the field of the same spelling
      ("total:", 0, None, "-: 0:16 declaration"),
      // a parameter; the loop variable is nearer in the loop
      ("scale:", 0, Some("entry"), "parameter: 2:30 declaration, 5:28 read | 5:28 shadowed"),
      // the local of a nested block
      ("scale = 2", 0, Some("total"), "variable: 6:10 declaration, 6:30 read | 6:21 captured"),
      // the variable of a for
      ("entry in", 0, None, "loop variable: 4:6 declaration, 5:13 read"),
      // a binding of a record pattern: the names a format string takes, the tokens and the name of a macro, a type
      (
        "read }",
        0,
        Some("sum"),
        "variable: 10:22 declaration destructured, 14:16 read macro, 14:43 read macro, 15:2 read namesake, 16:13 read namesake, 17:28 read | 13:33 captured, 18:22 captured",
      ),
      // a binding that is the name of its field
      ("total } = other", 0, None, "variable: 11:15 declaration shorthand, 12:29 read"),
      // a binding of a tuple pattern, a width of a format string
      ("first,", 0, None, "variable: 12:7 declaration destructured, 14:33 read macro, 15:8 read macro, 16:20 read"),
      // the parameter of a closure
      ("item:", 0, Some("sum"), "lambda parameter: 13:15 declaration, 13:26 read | 13:33 captured"),
      // the binding of an if let
      ("found)", 0, None, "variable: 17:14 declaration destructured, 17:43 read"),
      // the binding of an arm
      ("more =>", 0, None, "variable: 18:27 declaration, 18:35 read"),
    ],
  );
}

#[test]
fn typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::TypeScript,
    TYPESCRIPT,
    &[
      // a local with several uses: the property it names, the targets of two destructurings, a call, a type, a template
      (
        "total = 0\n    for",
        0,
        Some("sum"),
        "variable: 4:8 declaration, 6:6 write, 7:25 write, 9:36 write, 10:19 read shorthand, 13:8 write shorthand, 14:6 write destructured, 15:10 call, 17:17 read namesake, 17:28 read, 19:11 read | 15:47 captured, 16:40 captured",
      ),
      // the field of the same spelling, and the type
      ("total = 0", 0, None, "-: 1:2 declaration, 10:36 read, 23:5 declaration"),
      // a parameter of a method: no call names it
      ("scale: number)", 0, Some("entry"), "parameter: 3:28 declaration, 6:30 read, 9:24 read | 6:30 shadowed"),
      // the local of a nested block
      ("scale = 2", 0, Some("total"), "variable: 7:14 declaration, 7:34 read | 7:25 captured"),
      // the variable of a for
      ("entry of", 0, None, "loop variable: 5:15 declaration, 6:15 read"),
      // the variable of the head of a for
      ("i = 0", 0, None, "loop variable: 9:13 declaration, 9:20 read, 9:31 write"),
      // a binding that is the name of its property
      ("first", 0, None, "variable: 11:12 declaration shorthand"),
      // a binding of a pattern under another key
      ("renamed", 0, None, "variable: 11:27 declaration destructured"),
      // the rest of a pattern
      ("rest", 0, None, "variable: 11:39 declaration destructured"),
      // an element of an array pattern
      ("head", 0, None, "variable: 12:11 declaration destructured"),
      // the parameter of a catch
      ("error)", 0, None, "catch parameter: 15:27 declaration, 15:40 read"),
      // the parameter of an arrow function
      ("item)", 0, Some("sum"), "lambda parameter: 16:21 declaration, 16:34 read | 16:40 captured"),
      // a parameter that is the name of its property
      ("size }", 0, None, "parameter: 18:21 declaration shorthand, 18:62 read"),
      // a parameter of an array pattern
      ("width]", 0, None, "parameter: 18:35 declaration destructured, 18:69 read"),
      // a parameter with a modifier declares a property
      ("scale: number,", 0, None, "other: 2:22 declaration"),
      // a parameter of a constructor
      ("plain", 0, None, "parameter: 2:37 declaration, 2:58 read"),
      // a function inside a function
      ("inner", 0, None, "other: 18:13 declaration"),
    ],
  );
}

#[test]
fn tsx_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Tsx,
    TSX,
    &[
      // a local of an array pattern, read by two attributes of its spelling; `sum` is read in the element
      ("total,", 0, Some("sum"), "variable: 1:9 declaration destructured, 2:65 read, 3:23 read | 3:37 captured"),
      // a parameter that is the name of its property
      ("entries,", 0, None, "parameter: 0:25 declaration shorthand, 2:15 read"),
      // a parameter, and the property a shorthand names by it
      ("scale }", 0, None, "parameter: 0:34 declaration shorthand, 2:92 read shorthand"),
      // the parameter of an arrow function, beside an attribute of its spelling
      ("entry)", 0, Some("total"), "lambda parameter: 2:28 declaration, 2:48 read, 2:79 read | 2:65 captured"),
      // a local read in an element
      ("rows", 0, None, "variable: 2:8 declaration, 3:31 read"),
    ],
  );
}

#[test]
fn javascript_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::JavaScript,
    JAVASCRIPT,
    &[
      // a local with several uses: the property it names, a call
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 1:6 declaration, 3:4 write, 4:21 write, 6:42 read, 7:17 read shorthand, 8:8 call, 9:48 read | 8:45 captured",
      ),
      // a parameter
      ("scale)", 0, Some("entry"), "parameter: 0:26 declaration, 3:28 read | 3:28 shadowed"),
      // the local of a nested block
      ("scale = 2", 0, Some("total"), "variable: 4:10 declaration, 4:30 read | 4:21 captured"),
      // a binding that is the name of its property
      ("first", 0, None, "variable: 6:10 declaration shorthand"),
      // a binding of a pattern under another key
      ("renamed", 0, None, "variable: 6:25 declaration destructured"),
      // the parameter of a catch
      ("error)", 0, None, "catch parameter: 8:25 declaration, 8:38 read"),
      // the parameter of the inner function shadows the outer one
      ("scale) {", 1, None, "lambda parameter: 9:24 declaration, 9:40 read"),
      // the name of a function expression
      ("inner", 0, None, "other: 9:18 declaration"),
    ],
  );
}

#[test]
fn a_text_with_a_syntax_error_still_answers_the_names_its_tree_holds() {
  let cases: &[(Grammar, &str, &str)] = &[
    (
      Grammar::Kotlin,
      "fun f(scale: Int) {\n  val total = 1\n  val broken = (\n  use(total, scale)\n}\n",
      "variable: 1:6 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::Java,
      "class A { void f(int scale) {\n  int total = 1;\n  int broken = (;\n  use(total, scale);\n} }\n",
      "variable: 1:6 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::CSharp,
      "class A { void F(int scale) {\n  var total = 1;\n  var broken = (;\n  Use(total, scale);\n} }\n",
      "variable: 1:6 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::Starlark,
      "def f(scale):\n    total = 1\n    broken = (\n    use(total, scale)\n",
      "variable: 1:4 declaration, 3:8 read | 3:15 captured",
    ),
    (
      Grammar::Rust,
      "fn f(scale: u32) {\n  let total = 1;\n  let broken = (;\n  use_it(total, scale);\n}\n",
      "variable: 1:6 declaration, 3:9 read | 3:16 captured",
    ),
    (
      Grammar::TypeScript,
      "function f(scale: number) {\n  const total = 1\n  break\n  use(total, scale)\n}\n",
      "variable: 1:8 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::Tsx,
      "function f(scale: number) {\n  const total = 1\n  const broken = <div></span>\n  use(total, scale)\n}\n",
      "variable: 1:8 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::JavaScript,
      "function f(scale) {\n  const total = 1\n  const total = 2\n  use(total, scale)\n}\n",
      "variable: 1:8 declaration | 3:13 captured",
    ),
    (
      Grammar::TypeScript,
      "function f(scale: number) {\n  const total = 1\n  const broken = (;\n  use(total, scale)\n}\n",
      "none",
    ),
  ];
  for &(grammar, text, expected) in cases {
    // the local `total`, asked whether `scale` (the parameter its last line reads beside it) could name it; the last
    // row is a text oxc reads no program of: no name is answered rather than a guess
    assert_eq!(ask(grammar, text, "total", 0, Some("scale")), expected, "{grammar:?}\n{text}");
  }
}

/// Every start of a word of a text: the places a caller may ask, a name or not
fn word_starts(text: &str) -> Vec<Position> {
  let is_word = |it: char| it == '_' || it.is_alphanumeric();
  let mut starts = Vec::new();
  let mut previous = ' ';
  for (offset, character) in text.char_indices() {
    if is_word(character) && !is_word(previous) && !character.is_numeric() {
      starts.push(crate::navigate::position_of(text, offset));
    }
    previous = character;
  }
  starts
}

#[test]
fn many_places_asked_at_once_answer_as_each_asked_alone() {
  let texts = [
    (Grammar::Kotlin, KOTLIN),
    (Grammar::Kotlin, KOTLIN_KINDS),
    (Grammar::Java, JAVA),
    (Grammar::CSharp, CSHARP),
    (Grammar::Starlark, STARLARK),
    (Grammar::Rust, RUST),
    (Grammar::TypeScript, TYPESCRIPT),
    (Grammar::Tsx, TSX),
    (Grammar::JavaScript, JAVASCRIPT),
  ];
  for (grammar, text) in texts {
    let places = word_starts(text);
    // every other place asks what `sum` would clash with
    let questions: Vec<NameQuestion<'_>> = places
      .iter()
      .enumerate()
      .map(|(index, &position)| NameQuestion {
        position,
        renamed_to: (index % 2 == 0).then_some("sum"),
      })
      .collect();
    let together = names_at(grammar, text, &questions);
    assert_eq!(together.asked.len(), questions.len());
    let mut names = 0;
    for (index, question) in questions.iter().enumerate() {
      let alone = names_at(grammar, text, &[*question]);
      assert_eq!(together.of(index), alone.of(0), "{grammar:?} at {:?}", question.position);
      names += usize::from(alone.of(0).is_some());
      // Find Usages is the same answer without the sites
      let usages = usages_at(grammar, text, question.position);
      assert_eq!(
        usages.as_ref().map(|it| (it.name.as_str(), it.local)),
        alone.of(0).map(|it| (it.name.as_str(), it.local.is_some()))
      );
      assert_eq!(
        usages.map(|it| it.usages.iter().map(|it| (it.start, it.end_column, it.access)).collect::<Vec<_>>()),
        alone.of(0).map(|it| it.uses.iter().map(|it| (it.start, it.end_column, it.access)).collect::<Vec<_>>())
      );
    }
    assert!(names > 20, "{grammar:?}: {names} names of {} places", places.len());
    // the places of one binding asked alike share an entry: far fewer entries than names
    assert!(together.names.len() < names, "{grammar:?}: {} entries for {names} names", together.names.len());
  }
}

#[test]
fn the_places_of_one_binding_share_an_entry_and_a_place_of_no_name_has_none() {
  let text = "fun f(scale: Int) {\n  val total = scale\n  use(total, total, other)\n}\n";
  let at = |needle: &str, occurrence: usize, renamed_to: Option<&'static str>| NameQuestion {
    position: place(text, needle, occurrence),
    renamed_to,
  };
  let questions = [
    at("total", 0, None),
    at("total", 1, None),
    at("total", 2, Some("scale")),
    at("total", 1, Some("scale")),
    at("total", 2, Some("other")),
    at("other", 0, Some("total")),
    at("{", 0, None),
    NameQuestion::at(Position { line: 9, column: 0 }),
  ];
  let found = names_at(Grammar::Kotlin, text, &questions);
  // the three places of the local: one entry without a new name, one per new name; a name that is no local is asked
  // of no clash; no name at a brace or past the text
  assert_eq!(found.asked, [Some(0), Some(0), Some(1), Some(1), Some(2), Some(3), None, None]);
  let answers: Vec<String> = found.names.iter().map(describe).collect();
  assert_eq!(
    answers,
    [
      "variable: 1:6 declaration, 2:6 read, 2:13 read",
      "variable: 1:6 declaration, 2:6 read, 2:13 read | no clash",
      "variable: 1:6 declaration, 2:6 read, 2:13 read | 2:20 captured",
      "-: 2:20 read",
    ]
  );
  // a new name that is the name itself changes nothing
  assert_eq!(
    ask(Grammar::Kotlin, text, "total", 0, Some("total")),
    "variable: 1:6 declaration, 2:6 read, 2:13 read | no clash"
  );
  // no question, no parse
  assert_eq!(names_at(Grammar::Kotlin, text, &[]), crate::navigate::NamesAt::default());
}

/// The words of a text with the bytes they start at
fn words(text: &str) -> Vec<(usize, &str)> {
  let is_word = |it: char| it == '_' || it.is_alphanumeric();
  let mut found = Vec::new();
  let mut start = None;
  for (offset, character) in text.char_indices().chain([(text.len(), ' ')]) {
    match (start, is_word(character)) {
      (None, true) => start = Some(offset),
      (Some(from), false) => {
        found.push((from, &text[from..offset]));
        start = None;
      }
      _ => {}
    }
  }
  found
}

/// Whether the names at `sites` of a text are one local of a body and all of it, each a plain use, and `new_name` is
/// free where the local is seen: what a reader of a rename asks of each side of a change
fn is_one_local(grammar: Grammar, text: &str, sites: &[Position], new_name: &str) -> bool {
  let questions: Vec<NameQuestion<'_>> = sites
    .iter()
    .map(|&position| NameQuestion {
      position,
      renamed_to: Some(new_name),
    })
    .collect();
  let found = names_at(grammar, text, &questions);
  let Some(&Some(entry)) = found.asked.first() else { return false };
  let name = &found.names[entry];
  // the name of a named argument and a label are no uses of the local, whatever its scopes say
  let uses: Vec<_> = name.uses.iter().filter(|it| !matches!(it.site, Site::NamedArgument | Site::Label)).collect();
  found.asked.iter().all(|it| *it == Some(entry))
    && name.local.is_some_and(|it| it.kind.is_value() && !it.named_by_calls)
    && name.clashes.as_ref().is_some_and(Vec::is_empty)
    && uses.len() == sites.len()
    && uses.iter().zip(sites).all(|(used, site)| used.start == *site && used.site == Site::Plain)
    && uses.iter().any(|it| it.access == Access::Declaration)
}

/// Whether the change from `old` to `new`, two texts of the same words but `old_name` written `new_name` at some
/// places, reads as the rename of one local on both sides
fn is_rename(grammar: Grammar, old: &str, new: &str, old_name: &str, new_name: &str) -> bool {
  let (old_words, new_words) = (words(old), words(new));
  assert_eq!(old_words.len(), new_words.len(), "the texts differ by more than names:\n{old}\n{new}");
  // the words that differ: where the old name stood, and where the new one stands
  let pairs: Vec<_> = old_words.iter().zip(&new_words).filter(|pair| pair.0.1 != pair.1.1).collect();
  assert!(!pairs.is_empty() && pairs.iter().all(|it| it.0.1 == old_name && it.1.1 == new_name), "{pairs:?}");
  let old_sites: Vec<Position> = pairs.iter().map(|it| crate::navigate::position_of(old, it.0.0)).collect();
  let new_sites: Vec<Position> = pairs.iter().map(|it| crate::navigate::position_of(new, it.1.0)).collect();
  is_one_local(grammar, old, &old_sites, new_name) && is_one_local(grammar, new, &new_sites, old_name)
}

/// The cases a reader of renames must tell apart, read as `is_rename` reads them: the first of each language is the
/// rename of one local, every other changes what the code means or what other code may spell
#[test]
fn a_change_of_names_is_the_rename_of_one_local_only_where_both_sides_bind_alike() {
  let cases: &[(Grammar, &str, &str, bool, &str)] = &[
    (
      Grammar::Kotlin,
      "fun f() {\n  val total = 1\n  use(total, \"$total\")\n}\n",
      "fun f() {\n  val sum = 1\n  use(sum, \"$sum\")\n}\n",
      true,
      "a local and its uses",
    ),
    (
      Grammar::Kotlin,
      "val run = { name: String, label: String ->\n  val name = name.trim()\n  use(name)\n}\n",
      "val run = { name: String, label: String ->\n  val label = label.trim()\n  use(label)\n}\n",
      false,
      "the value of the variable reads another parameter now",
    ),
    (
      Grammar::Kotlin,
      "class A {\n  val sum = 0\n  fun f() {\n    val total = 1\n    use(total, sum)\n  }\n}\n",
      "class A {\n  val sum = 0\n  fun f() {\n    val sum = 1\n    use(sum, sum)\n  }\n}\n",
      false,
      "the new name takes a read of the property",
    ),
    (
      Grammar::Kotlin,
      "val run = { total: Int -> total() }\n",
      "val run = { sum: Int -> sum() }\n",
      false,
      "a call may be a function of the spelling",
    ),
    (
      Grammar::Kotlin,
      "fun f() {\n  val total = 1\n  g(total = total)\n}\n",
      "fun f() {\n  val sum = 1\n  g(sum = sum)\n}\n",
      false,
      "the named argument names another parameter now",
    ),
    (
      Grammar::Kotlin,
      "fun f() {\n  val total = 1\n  g(total = total)\n}\n",
      "fun f() {\n  val sum = 1\n  g(total = sum)\n}\n",
      true,
      "the named argument keeps its parameter",
    ),
    (
      Grammar::Kotlin,
      "fun f(total: Int) = total + 1\n",
      "fun f(sum: Int) = sum + 1\n",
      false,
      "a call may name the parameter",
    ),
    (
      Grammar::Java,
      "class A { int f(int total) { int next = total + 1; return next; } }\n",
      "class A { int f(int sum) { int next = sum + 1; return next; } }\n",
      true,
      "a parameter no call names",
    ),
    (
      Grammar::Java,
      "class A {\n  Box item, part;\n  void f() {\n    for (String item : item.parts()) { use(item); }\n  }\n}\n",
      "class A {\n  Box item, part;\n  void f() {\n    for (String part : part.parts()) { use(part); }\n  }\n}\n",
      false,
      "the loop runs over another field now",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = 1; Use(total, $\"{total}\"); } }\n",
      "class A { void F() { var sum = 1; Use(sum, $\"{sum}\"); } }\n",
      true,
      "a local and its uses",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int entry, item;\n  void F() {\n    foreach (var entry in xs) { Use(entry); }\n    Log(entry);\n  }\n}\n",
      "class A {\n  int entry, item;\n  void F() {\n    foreach (var item in xs) { Use(item); }\n    Log(item);\n  }\n}\n",
      false,
      "the statement after the loop logs another field now",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = 1; Use(nameof(total)); } }\n",
      "class A { void F() { var sum = 1; Use(nameof(sum)); } }\n",
      false,
      "the text of the name is a value",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = 1; Use(new { total }); } }\n",
      "class A { void F() { var sum = 1; Use(new { sum }); } }\n",
      false,
      "the member of the anonymous type has another name now",
    ),
    (
      Grammar::Starlark,
      "def f(xs):\n    total = len(xs)\n    return total\n",
      "def f(xs):\n    count = len(xs)\n    return count\n",
      true,
      "a local and its use",
    ),
    (
      Grammar::Starlark,
      "name = 1\nn = 2\ndef f(xs):\n    ys = [name for name in xs]\n    return name\n",
      "name = 1\nn = 2\ndef f(xs):\n    ys = [n for n in xs]\n    return n\n",
      false,
      "the function returns another name of the file now",
    ),
    (
      Grammar::Starlark,
      "name = 1\nlabel = 2\nkey = [lambda name = name: name]\n",
      "name = 1\nlabel = 2\nkey = [lambda label = label: label]\n",
      false,
      "the default reads another name of the file now",
    ),
    (
      Grammar::Rust,
      "fn f() {\n  let total = 1;\n  let total = total + 1;\n  use_it(total);\n}\n",
      "fn f() {\n  let total = 1;\n  let sum = total + 1;\n  use_it(sum);\n}\n",
      true,
      "the second binding and its use",
    ),
    (
      Grammar::Rust,
      "fn f(a: u32, b: u32) {\n  let a = a + 1;\n  use_it(a);\n}\n",
      "fn f(a: u32, b: u32) {\n  let b = b + 1;\n  use_it(b);\n}\n",
      false,
      "the value of the binding reads another parameter now",
    ),
    (
      Grammar::Rust,
      "fn f(o: Option<u32>, x: u32, y: u32) {\n  if let Some(x) = o { use_it(x); }\n  use_it(x);\n}\n",
      "fn f(o: Option<u32>, x: u32, y: u32) {\n  if let Some(y) = o { use_it(y); }\n  use_it(y);\n}\n",
      false,
      "the statement after the if let reads another parameter now",
    ),
    (
      Grammar::Rust,
      "fn f() {\n  let total = 1;\n  let s = S { total };\n}\n",
      "fn f() {\n  let sum = 1;\n  let s = S { sum };\n}\n",
      false,
      "the struct is given another field now",
    ),
    (
      Grammar::TypeScript,
      "function f() {\n  const total = 1\n  return `${total}` + total\n}\n",
      "function f() {\n  const sum = 1\n  return `${sum}` + sum\n}\n",
      true,
      "a local and its uses",
    ),
    (
      Grammar::TypeScript,
      "function f() {\n  const total = 1\n  return { total }\n}\n",
      "function f() {\n  const sum = 1\n  return { sum }\n}\n",
      false,
      "the object has another property now",
    ),
    (
      Grammar::TypeScript,
      "function f() {\n  const total = 1\n  return { total: total }\n}\n",
      "function f() {\n  const sum = 1\n  return { total: sum }\n}\n",
      true,
      "the property keeps its name",
    ),
    (
      Grammar::TypeScript,
      "function f(box: Box) {\n  const { total } = box\n  return total\n}\n",
      "function f(box: Box) {\n  const { sum } = box\n  return sum\n}\n",
      false,
      "another property is read now",
    ),
    (
      Grammar::Tsx,
      "function F() {\n  const total = 1\n  return <Row total={total} />\n}\n",
      "function F() {\n  const sum = 1\n  return <Row total={sum} />\n}\n",
      true,
      "the attribute keeps its name",
    ),
    (
      Grammar::JavaScript,
      "function f(xs) {\n  for (const item of xs) use(item)\n}\n",
      "function f(xs) {\n  for (const entry of xs) use(entry)\n}\n",
      true,
      "a loop variable",
    ),
    (
      Grammar::JavaScript,
      "var sum = 0\nfunction f() {\n  const total = 1\n  return total + sum\n}\n",
      "var sum = 0\nfunction f() {\n  const sum = 1\n  return sum + sum\n}\n",
      false,
      "the new name takes a read of the variable of the file",
    ),
  ];
  for &(grammar, old, new, expected, why) in cases {
    let (old_name, new_name) = words(old).iter().zip(&words(new)).find(|it| it.0.1 != it.1.1).map(|it| (it.0.1, it.1.1)).unwrap();
    assert_eq!(is_rename(grammar, old, new, old_name, new_name), expected, "{grammar:?}: {why}\n{old}\n{new}");
  }
}
