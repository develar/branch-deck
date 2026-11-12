//! The names of many places from one parse of a text (`navigate::names_at`,
//! openspec/specs/commit/name-bindings/spec.md): a table per language of what a name is bound to, what a body declares
//! it as, what each site makes of it and what another name for it would clash with

use crate::highlight::Grammar;
use crate::navigate::{
  Access, ClashKind, Lines, LocalBinding, LocalKind, NameAt, NameQuestion, Position, Site, byte_offset, names_at, names_at_within, resolution_work, usages_at,
};
use crate::test_support::place;
use pretty_assertions::assert_eq;

/// An answer as text: what declares the local (`-` for any other name), `named by calls` and `read in part` where it
/// says so, its uses as `line:column access site`, then after `|` the clashes of the new name asked, `no clash` where
/// it has none
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
      let named = if it.named_by_calls { " named by calls" } else { "" };
      let read = if it.read_in_part { " read in part" } else { "" };
      format!("{kind}{named}{read}")
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
    switch (scale) { case total: break; default: break; }
    Runnable task = new Runnable() { public void run() { use(total, scale); } };
    return total;
  }
  @GetMapping(USER) User get(@PathVariable String id) { return find(id); }
  @Override public String name(@NotNull String plain) { return plain; }
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
    var doubled = twice(total); var same = scale is total;
    again: scale--; if (scale > 0) goto again;
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
    if first:
        total = key(first)
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
  match second { 0 => sum, more => twice(more) }
}

const sum: u64 = 1;

#[instrument(skip(amount))]
fn load(amount: u64) -> u64 { amount }

#[inline]
#[cfg_attr(test, allow(unused))]
fn plain(count: u64) -> u64 { count }
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
      // a local with several uses: a call, a callable reference and a string template among them, not the name of the argument it is given as; `sum` is a property its body reads
      (
        "total = 0L",
        0,
        Some("sum"),
        "variable: 2:8 declaration, 4:6 write, 5:27 write, 7:31 read, 8:19 read, 9:10 call namesake, 9:52 read namesake, 9:61 read, 11:11 read | 10:44 captured",
      ),
      // the property of the same spelling: no use of the local is its; the name of the argument is a name of that text no local binds
      ("total: Long", 0, None, "-: 0:17 declaration, 8:11 read named argument, 8:37 read"),
      ("total = total", 0, Some("sum"), "-: 0:17 declaration, 8:11 read named argument, 8:37 read"),
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
      // a type of the spelling of a parameter is no use of the parameter
      ("handler:", 0, None, "parameter named by calls: 3:11 declaration, 8:25 read"),
      ("handler =", 0, None, "-: 8:15 read"),
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
      // a local with several uses; a call and a type of its spelling are none of them, a `case` label and a use in an anonymous class may be another thing; `sum` is read as a value, and called as the method
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 4:9 declaration, 6:6 write, 7:23 write, 9:38 write, 14:35 read, 16:26 read namesake, 17:61 read namesake, 18:11 read | 11:83 captured",
      ),
      // the field of the same spelling, with the call and the type no variable is
      ("total;", 0, None, "-: 1:7 declaration, 11:31 call, 11:76 read, 15:4 read"),
      // a parameter under no annotation: no call names it; its use in the anonymous class may be a member the class inherits
      (
        "scale)",
        0,
        Some("entry"),
        "parameter: 3:40 declaration, 6:30 read, 9:24 read, 16:12 read, 17:68 read namesake | 6:30 shadowed",
      ),
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
      // a parameter under an annotation of a framework, which may bind by its name
      ("id)", 0, None, "parameter named by calls: 20:50 declaration, 20:68 read"),
      // the annotations of the language and of nullability read no name
      ("plain)", 0, None, "parameter: 21:47 declaration, 21:63 read"),
    ],
  );
}

#[test]
fn csharp_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::CSharp,
    CSHARP,
    &[
      // a local with several uses: an element of a tuple and a member of an anonymous type it names, `nameof`, an interpolation, an argument of a call (of a method, of a delegate), whose text what is called may take, the right side of an `is` (a constant while no type has the name); not the member an initializer names, not a type
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 3:9 declaration, 5:6 write, 6:23 write, 8:38 write, 9:16 read shorthand, 10:21 read shorthand, 11:32 read, 12:18 read spelled, 13:22 read spelled, 13:34 read, 17:34 read, 19:24 read spelled, 19:52 read namesake, 21:11 read | 15:60 captured",
      ),
      // the field of the same spelling, with what no variable is: the member an initializer names, the named argument, the type
      ("total;", 0, None, "-: 1:7 declaration, 11:24 write, 12:11 read named argument, 12:35 read, 18:4 read"),
      // a parameter a call may name; the foreach variable is nearer in its loop only
      (
        "scale)",
        0,
        Some("entry"),
        "parameter named by calls: 2:40 declaration, 5:30 read, 8:24 read, 9:23 read shorthand, 10:35 read shorthand, 19:43 read, 20:11 write, 20:24 read | 5:30 shadowed",
      ),
      // the local of a nested block
      ("inner = 2", 0, Some("total"), "variable: 6:12 declaration, 6:32 read | 6:23 captured"),
      // the variable of a foreach
      ("entry in", 0, None, "loop variable: 4:17 declaration, 5:15 read"),
      // the variable of a for
      ("i = 0", 0, None, "loop variable: 8:13 declaration, 8:20 read, 8:31 write"),
      // a variable of a deconstruction
      ("first", 0, None, "variable: 14:9 declaration destructured"),
      // an out variable: declared in an argument
      ("seen", 0, None, "variable: 15:22 declaration spelled"),
      // the variable of a catch, given to a call
      ("e) { Log", 0, None, "catch parameter: 15:48 declaration, 15:57 read spelled"),
      // the parameter of a lambda whose body is an expression: an expression tree would hold its name
      ("item =>", 0, None, "lambda parameter: 16:27 declaration spelled, 16:35 read spelled"),
      // the parameter of a local function
      ("size)", 0, None, "parameter named by calls: 17:18 declaration, 17:27 read"),
      // a local of a setter; `value` is bound without a declaration
      ("size =", 0, Some("value"), "variable: 24:24 declaration | 24:24 implicit, 24:31 captured"),
      // a local function
      ("Local", 0, None, "other: 17:8 declaration"),
      // a local that holds a delegate: its call is a plain use
      ("twice =", 0, None, "variable: 16:19 declaration, 19:18 call"),
      // a label and its jump: no scope binds them
      ("again:", 0, Some("total"), "other: 20:4 declaration label, 20:40 read label | no clash"),
    ],
  );
}

#[test]
fn starlark_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Starlark,
    STARLARK,
    &[
      // a local with several uses, a later assignment of it among them, not the names of the arguments it is given as; `size` is a name of the file its body calls
      (
        "total = len",
        0,
        Some("size"),
        "variable: 3:4 declaration, 5:8 read, 5:35 read, 6:25 read, 8:30 read, 9:56 read, 13:8 write, 14:26 read | 5:17 captured, 9:70 captured",
      ),
      // the name of the file of the same spelling, with the names of the arguments
      ("total", 0, None, "-: 0:0 declaration, 9:48 read named argument, 14:18 read named argument"),
      // a parameter a call may name, apart from the variable of the comprehension
      ("name,", 0, None, "parameter named by calls: 2:13 declaration, 9:29 read"),
      // a parameter; the name of the loop variable would be the same variable of the def
      (
        "deps =",
        0,
        Some("dep"),
        "parameter named by calls: 2:19 declaration, 3:16 read, 4:15 read, 7:30 read, 9:42 read | 4:8 captured, 5:22 captured",
      ),
      // the variable of a for
      ("dep in", 0, None, "loop variable: 4:8 declaration, 5:22 read"),
      // a name a tuple of targets assigns
      ("first", 0, None, "variable: 6:4 declaration destructured, 12:7 read, 13:20 read"),
      // the parameter of a lambda, which a call may name as the one of a def
      ("item:", 0, None, "lambda parameter named by calls: 8:17 declaration, 8:23 read"),
      // no call names what `*args` takes
      ("args", 0, None, "parameter: 2:31 declaration, 14:40 read"),
      // a def inside a def
      ("inner", 0, None, "other: 10:8 declaration"),
      // the parameter of the inner def shadows the local
      ("total):", 0, None, "parameter named by calls: 10:14 declaration, 11:15 read"),
      // the variable of a comprehension, seen in all of it
      ("name for", 0, None, "loop variable: 7:13 read, 7:22 declaration, 7:38 read"),
      // a local that holds a lambda: its call is a plain use
      ("key =", 0, None, "variable: 8:4 declaration, 13:16 call"),
      // the name of a named argument is a name of that text no local binds
      (
        "total = total, size",
        0,
        Some("size"),
        "-: 0:0 declaration, 9:48 read named argument, 14:18 read named argument",
      ),
    ],
  );
}

#[test]
fn rust_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::Rust,
    RUST,
    &[
      // a local with several uses: the field it names by its value, not the name of a field before its colon; the binding of line 11 takes the later ones; `sum` is a constant its body reads
      (
        "total = 0",
        0,
        Some("sum"),
        "variable: 3:10 declaration, 5:4 write, 6:21 write, 8:24 read shorthand, 9:30 read | 13:33 captured, 18:22 captured",
      ),
      // the field of the same spelling, with its names before a colon
      ("total:", 0, None, "-: 0:16 declaration, 9:23 read, 10:15 read"),
      // a parameter; the loop variable is nearer in the loop
      ("scale:", 0, Some("entry"), "parameter: 2:30 declaration, 5:28 read | 5:28 shadowed"),
      // the local of a nested block
      ("scale = 2", 0, Some("total"), "variable: 6:10 declaration, 6:30 read | 6:21 captured"),
      // the variable of a for
      ("entry in", 0, None, "loop variable: 4:6 declaration, 5:13 read"),
      // a binding of a record pattern: the names a format string and the tokens of a macro take; not the name of a macro, not a type
      (
        "read }",
        0,
        Some("sum"),
        "variable: 10:22 declaration destructured, 14:16 read macro, 14:43 read macro, 17:28 read | 13:33 captured, 18:22 captured",
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
      ("more =>", 0, None, "variable: 18:27 declaration, 18:41 read"),
      // a local that holds a closure: its call is a plain use
      ("twice =", 0, None, "variable: 13:6 declaration, 18:35 call"),
      // the field before its colon is a member no scope binds
      ("total: total", 0, Some("sum"), "-: 0:16 declaration, 9:23 read, 10:15 read"),
      // an attribute macro reads the parameters of the function it stands on, as this one names one among its tokens
      ("amount:", 0, None, "parameter named by calls: 23:18 read macro, 24:8 declaration, 24:30 read"),
      // the attributes of the language read none
      ("count:", 0, None, "parameter: 28:9 declaration, 28:30 read"),
    ],
  );
}

#[test]
fn typescript_names_are_bound_by_their_scopes_with_their_sites_and_clashes() {
  check(
    Grammar::TypeScript,
    TYPESCRIPT,
    &[
      // a local with several uses: the property it names, the targets of two destructurings, a call, a template; not the type of its spelling
      (
        "total = 0\n    for",
        0,
        Some("sum"),
        "variable: 4:8 declaration, 6:6 write, 7:25 write, 9:36 write, 10:19 read shorthand, 13:8 write shorthand, 14:6 write destructured, 15:10 call, 17:28 read, 19:11 read | 15:47 captured, 16:40 captured",
      ),
      // the field of the same spelling, and the type with its use
      ("total = 0", 0, None, "-: 1:2 declaration, 10:36 read, 17:17 read, 23:5 declaration"),
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
      // a parameter with a modifier declares a property: no local
      ("scale: number,", 0, None, "-: 2:22 declaration"),
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
      "variable read in part: 1:6 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::Java,
      "class A { void f(int scale) {\n  int total = 1;\n  int broken = (;\n  use(total, scale);\n} }\n",
      "variable read in part: 1:6 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::CSharp,
      "class A { void F(int scale) {\n  var total = 1;\n  var broken = (;\n  Use(total, scale);\n} }\n",
      "variable read in part: 1:6 declaration, 3:6 read spelled | 3:13 captured",
    ),
    (
      Grammar::Starlark,
      "def f(scale):\n    total = 1\n    broken = (\n    use(total, scale)\n",
      // in a def the new name is the variable of the parameter: the names of the local would be its
      "variable read in part: 1:4 declaration, 3:8 read | 1:4 shadowed, 3:8 shadowed",
    ),
    (
      Grammar::Rust,
      "fn f(scale: u32) {\n  let total = 1;\n  let broken = (;\n  use_it(total, scale);\n}\n",
      "variable read in part: 1:6 declaration, 3:9 read | 3:16 captured",
    ),
    (
      Grammar::TypeScript,
      // an early error (a `break` outside a loop, a name declared twice) leaves the tree whole: every name is read
      "function f(scale: number) {\n  const total = 1\n  break\n  use(total, scale)\n}\n",
      "variable: 1:8 declaration, 3:6 read | 3:13 captured",
    ),
    (
      Grammar::Tsx,
      "function f(scale: number) {\n  const total = 1\n  const broken = <div></span>\n  use(total, scale)\n}\n",
      "variable read in part: 1:8 declaration, 3:6 read | 3:13 captured",
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
  // the local `total`, asked whether `scale` (the parameter its last line reads beside it) could name it: each answer
  // says that the text is read in part. The last row is a text oxc reads no program of: no name is answered rather
  // than a guess
  let found: Vec<_> = cases
    .iter()
    .map(|&(grammar, text, _)| (grammar, text, ask(grammar, text, "total", 0, Some("scale"))))
    .collect();
  let expected: Vec<_> = cases.iter().map(|&(grammar, text, answer)| (grammar, text, answer.to_string())).collect();
  assert_eq!(found, expected);
}

/// A text of a grammar, the `occurrence`-th `needle` of it, a new name for it or none, and its answer
type TextRow<'a> = (Grammar, &'a str, &'a str, usize, Option<&'a str>, &'a str);

/// Each row against the answer of its place
fn check_texts(rows: &[TextRow<'_>]) {
  let found: Vec<_> = rows
    .iter()
    .map(|&(grammar, text, needle, occurrence, renamed_to, _)| (text, ask(grammar, text, needle, occurrence, renamed_to)))
    .collect();
  let expected: Vec<_> = rows.iter().map(|&(_, text, _, _, _, answer)| (text, answer.to_string())).collect();
  assert_eq!(found, expected);
}

/// A local says that the scopes read only a part of the code that may name it: in a C# file with an `#if`, whatever
/// branch the parser takes; in a Rust function or closure with a `#[cfg]` inside it; in a script that calls `eval` by
/// its name. A condition on the whole function, one in another function and a call of a member named `eval` say
/// nothing of the local
#[test]
fn a_local_says_when_the_scopes_read_its_code_in_part() {
  check_texts(&[
    // a use the parser does not read: the text of the branch is no code
    (
      Grammar::CSharp,
      "class A {\n  int F() {\n    int total = 1;\n#if DEBUG\n    Log(total);\n#endif\n    return total;\n  }\n}\n",
      "total = 1",
      0,
      Some("sum"),
      "variable read in part: 2:8 declaration, 6:11 read | no clash",
    ),
    // the branch taken is no code where the other is compiled
    (
      Grammar::CSharp,
      "class A {\n  int F() {\n#if !DEBUG\n    int total = 1;\n#else\n    int total = 2;\n#endif\n    return total;\n  }\n}\n",
      "total = 1",
      0,
      None,
      "variable read in part: 3:8 declaration, 7:11 read",
    ),
    // a condition anywhere in the file: a brace in a branch may move every scope
    (
      Grammar::CSharp,
      "#if NET8_0\nusing System.Text;\n#endif\nclass A {\n  int F() {\n    int total = 1;\n    return total;\n  }\n}\n",
      "total = 1",
      0,
      None,
      "variable read in part: 5:8 declaration, 6:11 read",
    ),
    // another directive is no condition
    (
      Grammar::CSharp,
      "#nullable enable\nclass A {\n  int F() {\n#region body\n    int total = 1;\n    return total;\n#endregion\n  }\n}\n",
      "total = 1",
      0,
      None,
      "variable: 4:8 declaration, 5:11 read",
    ),
    // two declarations by two conditions are one scope to the reading, the second hiding the first
    (
      Grammar::Rust,
      "fn f() -> u32 {\n  #[cfg(unix)]\n  let total = 1;\n  #[cfg(windows)]\n  let total = 2;\n  total\n}\n",
      "total = 2",
      0,
      Some("sum"),
      "variable read in part: 4:6 declaration, 5:2 read | no clash",
    ),
    // a condition on a parameter, on an expression of a closure, under `cfg_attr`
    (
      Grammar::Rust,
      "fn f(#[cfg(unix)] total: u32, #[cfg(windows)] total: u64) -> u64 {\n  total.into()\n}\n",
      "total: u64",
      0,
      None,
      "parameter read in part: 0:46 declaration, 1:2 read",
    ),
    (
      Grammar::Rust,
      "static RUN: fn() -> u32 = || {\n  let total = 1;\n  #[cfg(test)]\n  log(total);\n  total\n};\n",
      "total = 1",
      0,
      None,
      "variable read in part: 1:6 declaration, 3:6 read, 4:2 read",
    ),
    (
      Grammar::Rust,
      "fn f() -> u32 {\n  let total = 1;\n  #[cfg_attr(test, cfg(unix))]\n  log(total);\n  total\n}\n",
      "total = 1",
      0,
      None,
      "variable read in part: 1:6 declaration, 3:6 read, 4:2 read",
    ),
    // a condition on the function compiles all of it or nothing; one in another function is not its
    (
      Grammar::Rust,
      "#[cfg(unix)]\nfn f() -> u32 {\n  let total = 1;\n  total\n}\n\nfn g() {\n  #[cfg(test)]\n  log(1);\n}\n",
      "total = 1",
      0,
      None,
      "variable: 2:6 declaration, 3:2 read",
    ),
    // `cfg_attr` without a `cfg` among what it adds, and `cfg!`, compile every line
    (
      Grammar::Rust,
      "fn f() -> u32 {\n  let total = 1;\n  #[cfg_attr(test, allow(unused))]\n  let other = cfg!(unix);\n  total\n}\n",
      "total = 1",
      0,
      None,
      "variable: 1:6 declaration, 4:2 read",
    ),
    // the text a direct `eval` runs names the variables around its call
    (
      Grammar::JavaScript,
      "function f(code) {\n  const total = 1\n  keep(total)\n  return eval(code)\n}\n",
      "total = 1",
      0,
      Some("sum"),
      "variable read in part: 1:8 declaration, 2:7 read | no clash",
    ),
    (
      Grammar::TypeScript,
      "function f(code: string) {\n  const total = 1\n  keep(total)\n  return () => (eval)(code)\n}\n",
      "total = 1",
      0,
      None,
      "variable read in part: 1:8 declaration, 2:7 read",
    ),
    // a member named `eval`, and the word in a string, run no text here
    (
      Grammar::JavaScript,
      "function f(code) {\n  const total = 1\n  keep(total, 'eval')\n  return vm.eval(code)\n}\n",
      "total = 1",
      0,
      None,
      "variable: 1:8 declaration, 2:7 read",
    ),
  ]);
}

/// A name whose spelling is a value where the code runs says so at that place: the declaration of a Kotlin property
/// with a delegate, which gets the name; a variable or a parameter of TypeScript and JavaScript that is given a
/// function or a class written without a name, which takes it. Inside the body of a `with` a name is a property of
/// the object first: a namesake
#[test]
fn a_name_that_is_handed_on_as_text_is_spelled_where_it_is() {
  check_texts(&[
    (
      Grammar::Kotlin,
      "fun f(m: Map<String, Int>): Int {\n  val total by m\n  println(total)\n  return total\n}\n",
      "total by",
      0,
      Some("sum"),
      "variable: 1:6 declaration spelled, 2:10 read, 3:9 read | no clash",
    ),
    (
      Grammar::KotlinScript,
      "tasks {\n  val integrationTest by registering(Test::class) {\n    group = \"verification\"\n  }\n  check {\n    dependsOn(integrationTest)\n  }\n}\n",
      "integrationTest by",
      0,
      None,
      "variable: 1:6 declaration spelled, 5:14 read",
    ),
    // a delegate of a member is its declaration too; a value after `=` gets no name
    (
      Grammar::Kotlin,
      "class A {\n  val total by lazy { 1 }\n  fun f(): Int {\n    val other = lazy { 2 }\n    return total + other.value\n  }\n}\n",
      "total by",
      0,
      None,
      "-: 1:6 declaration spelled, 4:11 read",
    ),
    (
      Grammar::Kotlin,
      "class A {\n  val total by lazy { 1 }\n  fun f(): Int {\n    val other = lazy { 2 }\n    return total + other.value\n  }\n}\n",
      "other =",
      0,
      None,
      "variable: 3:8 declaration, 4:19 read",
    ),
    // a function and a class without a name take the name of the variable they are the value of
    (
      Grammar::TypeScript,
      "function f() {\n  const total = () => 1\n  log(total)\n  return total\n}\n",
      "total =",
      0,
      Some("sum"),
      "variable: 1:8 declaration spelled, 2:6 read, 3:9 read | no clash",
    ),
    (
      Grammar::TypeScript,
      "function f() {\n  const total = (function () { return 1 }) as Fn\n  return total\n}\n",
      "total =",
      0,
      None,
      "variable: 1:8 declaration spelled, 2:9 read",
    ),
    (
      Grammar::JavaScript,
      "function f() {\n  let Total = class {}\n  return Total\n}\n",
      "Total =",
      0,
      None,
      "variable: 1:6 declaration spelled, 2:9 read",
    ),
    // given later, by an assignment that names it; and as the default of a parameter
    (
      Grammar::JavaScript,
      "function f() {\n  let total\n  total = () => 1\n  total ??= function () {}\n  total += 1\n  return total\n}\n",
      "total\n",
      0,
      None,
      "variable: 1:6 declaration, 2:2 write spelled, 3:2 write spelled, 4:2 write, 5:9 read",
    ),
    (
      Grammar::TypeScript,
      "function f(total = () => 1, other = 2) {\n  return total() + other\n}\n",
      "total =",
      0,
      None,
      "parameter: 0:11 declaration spelled, 1:9 call",
    ),
    // a function with a name of its own, a call that gives a function, and a pattern take none
    (
      Grammar::TypeScript,
      "function f() {\n  const total = function named() {}\n  const made = make(() => 1)\n  return [total, made]\n}\n",
      "total =",
      0,
      None,
      "variable: 1:8 declaration, 3:10 read",
    ),
    (
      Grammar::TypeScript,
      "function f() {\n  const total = function named() {}\n  const made = make(() => 1)\n  return [total, made]\n}\n",
      "made =",
      0,
      None,
      "variable: 2:8 declaration, 3:17 read",
    ),
    // in a `with`, a property of the object comes before the variable
    (
      Grammar::JavaScript,
      "function f(o) {\n  const total = 1\n  with (o) {\n    use(total)\n  }\n  return total\n}\n",
      "total = 1",
      0,
      Some("sum"),
      "variable: 1:8 declaration, 3:8 read namesake, 5:9 read | no clash",
    ),
    // and a name the `with` reads would be the variable's under the new name
    (
      Grammar::JavaScript,
      "function f(o) {\n  const total = 1\n  with (o) {\n    use(sum)\n  }\n  return total\n}\n",
      "total = 1",
      0,
      Some("sum"),
      "variable: 1:8 declaration, 5:9 read | 3:8 captured",
    ),
  ]);
}

/// A parameter of a TypeScript constructor with a modifier declares a property of its class: no local, and one of
/// the names of its spelling that no local binds, with `this.total`. Inside a function the class and its property are
/// its locals
#[test]
fn a_parameter_that_declares_a_property_is_a_member_of_its_class() {
  check_texts(&[
    (
      Grammar::TypeScript,
      "export class A {\n  constructor(public total: number, plain: number) {\n    use(total, plain)\n  }\n  f() { return this.total + 1 }\n}\n",
      "total:",
      0,
      Some("sum"),
      "-: 1:21 declaration, 2:8 read, 4:20 read",
    ),
    (
      Grammar::TypeScript,
      "export class A {\n  constructor(public total: number, plain: number) {\n    use(total, plain)\n  }\n  f() { return this.total + 1 }\n}\n",
      "plain:",
      0,
      None,
      "parameter: 1:36 declaration, 2:15 read",
    ),
    (
      Grammar::TypeScript,
      "class A {\n  constructor(private readonly total: number) {}\n}\nclass B {\n  constructor(readonly total = 1) {}\n}\n",
      "total:",
      0,
      None,
      "-: 1:31 declaration, 4:23 declaration",
    ),
    (
      Grammar::TypeScript,
      "function make() {\n  class A {\n    constructor(public total: number) {}\n    f() { return this.total }\n  }\n  return A\n}\n",
      "total:",
      0,
      None,
      "other: 2:23 declaration",
    ),
  ]);
}

/// Every start of a word of a text, and the end of every third one: the places a caller may ask, a name or not
/// (the end of a name is still the name)
fn word_starts(text: &str) -> Vec<Position> {
  let is_word = |it: char| it == '_' || it.is_alphanumeric();
  let mut starts = Vec::new();
  let mut previous = ' ';
  let mut words = 0;
  for (offset, character) in text.char_indices() {
    if is_word(character) && !is_word(previous) && !character.is_numeric() {
      starts.push(crate::navigate::position_of(text, offset));
      words += 1;
    } else if !is_word(character) && is_word(previous) && words % 3 == 0 {
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

/// The bytes of many places of a text are found from its lines read once (`Lines`), as `byte_offset` finds each
/// from the start of the text: at every line and column of a text with `\n`, `\r\n`, a lone `\r`, a character of
/// two UTF-16 units and no last line end, past the end of a line and past the last line
#[test]
fn the_bytes_of_many_places_are_those_of_each_place() {
  for text in ["", "a\n", "fun f() {\r\n  val \u{1d4b3} = 1\rreturn x\n\n}", "\r\r\n\n\r", "a\r\nb\rc\nd"] {
    let lines = Lines::of(text);
    for line in 0..8 {
      for column in 0..16 {
        let position = Position { line, column };
        assert_eq!(lines.byte_offset(position), byte_offset(text, position), "{text:?} at {position:?}");
      }
    }
  }
}

/// The places are answered within a number of resolutions, one per spelling asked and one more per binding asked
/// with another name, in the order asked: a place whose answer needs a resolution past them has none, a place of a
/// binding answered before it has that answer, and the answers within the limit are those without one
#[test]
fn the_places_are_answered_within_a_number_of_resolutions() {
  let text = "fun f(scale: Int) {\n  val total = scale\n  use(total, total, other)\n}\n\nfun g() {\n  val total = 1\n  use(total)\n}\n";
  let at = |needle: &str, occurrence: usize, renamed_to: Option<&'static str>| NameQuestion {
    position: place(text, needle, occurrence),
    renamed_to,
  };
  // the local of `f` at two places, the local of `g`, a name no local binds, and `scale` with no new name
  let questions = [
    at("total", 0, Some("sum")),
    at("total", 2, Some("sum")),
    at("total", 3, Some("sum")),
    at("other", 0, Some("sum")),
    at("scale", 0, None),
    at("total", 4, Some("sum")),
  ];
  let whole = names_at(Grammar::Kotlin, text, &questions);
  // the spelling `total`, the clashes of the two locals, the spellings `other` and `scale`
  assert_eq!((&whole.asked[..], whole.resolutions), (&[Some(0), Some(0), Some(1), Some(2), Some(3), Some(1)][..], 5));
  assert_eq!(names_at_within(Grammar::Kotlin, text, &questions, 5), whole);
  for (limit, asked) in [
    (0, [None; 6]),
    // the spelling alone: the clashes of no local
    (1, [None; 6]),
    (2, [Some(0), Some(0), None, None, None, None]),
    // the second local of the spelling; then a place of it asked again has its answer
    (3, [Some(0), Some(0), Some(1), None, None, Some(1)]),
    (4, [Some(0), Some(0), Some(1), Some(2), None, Some(1)]),
  ] {
    let found = names_at_within(Grammar::Kotlin, text, &questions, limit);
    assert_eq!((found.asked.as_slice(), found.resolutions), (&asked[..], limit), "within {limit}");
    assert_eq!(found.names, whole.names[..found.names.len()], "within {limit}");
  }
  // a walk of the tree is the whole of the tokens of a text, a read of the names of one spelling an eighth
  assert_eq!(
    [Grammar::Java, Grammar::CSharp, Grammar::Starlark, Grammar::Kotlin, Grammar::Rust, Grammar::TypeScript].map(|grammar| resolution_work(grammar, 800)),
    [800, 800, 800, 100, 100, 100]
  );
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
  found.asked.iter().all(|it| *it == Some(entry))
    && name.local.is_some_and(LocalBinding::is_of_body_alone)
    && name.clashes.as_ref().is_some_and(Vec::is_empty)
    && name.uses.len() == sites.len()
    && name.uses.iter().zip(sites).all(|(used, site)| used.start == *site && used.site == Site::Plain)
    && name.uses.iter().any(|it| it.access == Access::Declaration)
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

/// The cases a reader of renames must tell apart, read as `is_rename` reads them: a rename of one local, or a change
/// of what the code means or of what other code may spell
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
      Grammar::Kotlin,
      "fun f(): String {\n  val total = \"local\"\n  val o = object { val sum = \"member\"; fun g() = sum }\n  return o.g() + total\n}\n",
      "fun f(): String {\n  val sum = \"local\"\n  val o = object { val sum = \"member\"; fun g() = sum }\n  return o.g() + sum\n}\n",
      false,
      "the function of the object reads the variable now, not its property",
    ),
    (
      Grammar::Kotlin,
      "class A {\n  var total = 0\n  fun f(): Int {\n    var n = 0\n    do {\n      val total = 500\n      n += total\n    } while (total > 100 && n < 2000)\n    return n\n  }\n}\n",
      "class A {\n  var total = 0\n  fun f(): Int {\n    var n = 0\n    do {\n      val sum = 500\n      n += sum\n    } while (total > 100 && n < 2000)\n    return n\n  }\n}\n",
      false,
      "the condition of the loop reads the property now",
    ),
    (
      Grammar::Kotlin,
      "fun f(): Int {\n  val size = size()\n  return size + 1\n}\n",
      "fun f(): Int {\n  val count = size()\n  return count + 1\n}\n",
      true,
      "the function that gives the value keeps its name",
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
      Grammar::Java,
      "class A { int f() { int size = size(); return size + 1; } }\n",
      "class A { int f() { int count = size(); return count + 1; } }\n",
      true,
      "the method that gives the value keeps its name: a call is of no variable",
    ),
    (
      Grammar::Java,
      "class A {\n  String f() {\n    String total = \"local\";\n    return new Base() { String g() { return total; } }.g();\n  }\n}\n",
      "class A {\n  String f() {\n    String sum = \"local\";\n    return new Base() { String g() { return sum; } }.g();\n  }\n}\n",
      false,
      "a field the anonymous class inherits may take the read",
    ),
    (
      Grammar::Java,
      "class A {\n  String f() {\n    String total = \"local\";\n    class Box extends Base { String g() { return total; } }\n    return new Box().g();\n  }\n}\n",
      "class A {\n  String f() {\n    String sum = \"local\";\n    class Box extends Base { String g() { return sum; } }\n    return new Box().g();\n  }\n}\n",
      false,
      "a field the local class inherits may take the read",
    ),
    (
      Grammar::Java,
      "class A { int f(Kind kind) { int total = 1; switch (kind) { case total: return total; default: return 0; } } }\n",
      "class A { int f(Kind kind) { int sum = 1; switch (kind) { case sum: return sum; default: return 0; } } }\n",
      false,
      "the label may be a constant of the enum",
    ),
    (
      Grammar::Java,
      "class A {\n  Res total, sum;\n  String f(Object o) {\n    if (o instanceof Res total) { return \"then \" + total; } else { return \"else \" + total; }\n  }\n}\n",
      "class A {\n  Res total, sum;\n  String f(Object o) {\n    if (o instanceof Res sum) { return \"then \" + sum; } else { return \"else \" + sum; }\n  }\n}\n",
      false,
      "the else reads another field now: the variable of the pattern is not in it",
    ),
    (
      Grammar::Java,
      "class A {\n  Res total, sum;\n  String f(Object o) {\n    if (!(o instanceof Res total)) { return \"then \" + total; }\n    return \"after \" + total;\n  }\n}\n",
      "class A {\n  Res total, sum;\n  String f(Object o) {\n    if (!(o instanceof Res sum)) { return \"then \" + sum; }\n    return \"after \" + sum;\n  }\n}\n",
      false,
      "the branch of the test that did not match reads another field now",
    ),
    (
      Grammar::Java,
      "class A {\n  Res total, sum;\n  boolean f(Object o) { return o instanceof Res total || total.ok(); }\n}\n",
      "class A {\n  Res total, sum;\n  boolean f(Object o) { return o instanceof Res sum || sum.ok(); }\n}\n",
      false,
      "the operand after || reads another field now",
    ),
    (
      Grammar::Java,
      "class A {\n  Res total, sum;\n  String f(Object o) { return o instanceof Res total ? \"t \" + total : \"f \" + total; }\n}\n",
      "class A {\n  Res total, sum;\n  String f(Object o) { return o instanceof Res sum ? \"t \" + sum : \"f \" + sum; }\n}\n",
      false,
      "the third operand reads another field now",
    ),
    (
      Grammar::Java,
      "class A {\n  boolean f(Object o) {\n    if (!(o instanceof String total)) return false;\n    return total.isEmpty();\n  }\n}\n",
      "class A {\n  boolean f(Object o) {\n    if (!(o instanceof String sum)) return false;\n    return sum.isEmpty();\n  }\n}\n",
      true,
      "the variable of a pattern and its use after the statement that leaves without it",
    ),
    (
      Grammar::Java,
      "class A {\n  Res total, sum;\n  void f(StringBuilder b) {\n    try (Res total = open()) { b.append(total); }\n    finally { b.append(total); }\n  }\n}\n",
      "class A {\n  Res total, sum;\n  void f(StringBuilder b) {\n    try (Res sum = open()) { b.append(sum); }\n    finally { b.append(sum); }\n  }\n}\n",
      false,
      "the finally reads another field now: the resource is not in it",
    ),
    (
      Grammar::Java,
      "class A {\n  Res total, sum;\n  void f(StringBuilder b) {\n    try (Res total = open()) { b.append(total); }\n    catch (RuntimeException e) { b.append(total); }\n  }\n}\n",
      "class A {\n  Res total, sum;\n  void f(StringBuilder b) {\n    try (Res sum = open()) { b.append(sum); }\n    catch (RuntimeException e) { b.append(sum); }\n  }\n}\n",
      false,
      "the catch reads another field now",
    ),
    (
      Grammar::Java,
      "class A { void f() { try (Res total = open()) { use(total); } finally { done(); } } }\n",
      "class A { void f() { try (Res sum = open()) { use(sum); } finally { done(); } } }\n",
      true,
      "a resource and its use in the block of the try",
    ),
    (
      Grammar::Java,
      "class Users {\n  @GetMapping(\"/users/{id}\")\n  User get(@PathVariable String id) {\n    return repo.find(id);\n  }\n}\n",
      "class Users {\n  @GetMapping(\"/users/{id}\")\n  User get(@PathVariable String userId) {\n    return repo.find(userId);\n  }\n}\n",
      false,
      "the framework binds the variable of the path by the name of the parameter",
    ),
    (
      Grammar::Java,
      "interface Users {\n  @Query(\"select u from User u where u.name = :name\")\n  User find(String name);\n}\n",
      "interface Users {\n  @Query(\"select u from User u where u.name = :name\")\n  User find(String label);\n}\n",
      false,
      "the query names the parameter",
    ),
    (
      Grammar::Java,
      "@Service class Users { User get(String id) { return repo.find(id); } }\n",
      "@Service class Users { User get(String key) { return repo.find(key); } }\n",
      false,
      "a framework may read the parameters of a class it manages",
    ),
    (
      Grammar::Java,
      "class Users { @Override public User get(@NotNull String id) { return repo.find(id); } }\n",
      "class Users { @Override public User get(@NotNull String key) { return repo.find(key); } }\n",
      true,
      "the annotations of the language and of nullability read no name",
    ),
    (
      Grammar::CSharp,
      "class A { string F() { var total = 1; total += 2; return $\"{total}\"; } }\n",
      "class A { string F() { var sum = 1; sum += 2; return $\"{sum}\"; } }\n",
      true,
      "a local and its uses",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = Load(); ArgumentNullException.ThrowIfNull(total); } }\n",
      "class A { void F() { var sum = Load(); ArgumentNullException.ThrowIfNull(sum); } }\n",
      false,
      "what is called may take the text of its argument: the exception names another parameter now",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = Load(); Debug.Assert(total > 0); } }\n",
      "class A { void F() { var sum = Load(); Debug.Assert(sum > 0); } }\n",
      false,
      "the failed assertion says another text now",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = Load(); total.ShouldBe(1); } }\n",
      "class A { void F() { var sum = Load(); sum.ShouldBe(1); } }\n",
      false,
      "an extension method may take the text of what it is called on",
    ),
    (
      Grammar::CSharp,
      "class A { void F() { var total = Load(); total?.Items.Check(); } }\n",
      "class A { void F() { var sum = Load(); sum?.Items.Check(); } }\n",
      false,
      "so may one called after `?.`",
    ),
    (
      Grammar::CSharp,
      "class A { int F(int[] items) { var total = Load(); return items[total]; } }\n",
      "class A { int F(int[] items) { var sum = Load(); return items[sum]; } }\n",
      false,
      "an indexer may take the text of its argument",
    ),
    (
      Grammar::CSharp,
      "class A { object F() { Expression<Func<Row, bool>> ready = row => row.Ok; return ready; } }\n",
      "class A { object F() { Expression<Func<Row, bool>> ready = line => line.Ok; return ready; } }\n",
      false,
      "the expression tree of the lambda holds the name of its parameter",
    ),
    (
      Grammar::CSharp,
      "class A { object F() { var limit = Load(); Expression<Func<Row, bool>> ready = row => row.Size < limit; return ready; } }\n",
      "class A { object F() { var bound = Load(); Expression<Func<Row, bool>> ready = row => row.Size < bound; return ready; } }\n",
      false,
      "and the name of a variable around it",
    ),
    (
      Grammar::CSharp,
      "class A { object F() { Func<int, int> twice = item => { return item * 2; }; return twice; } }\n",
      "class A { object F() { Func<int, int> twice = part => { return part * 2; }; return twice; } }\n",
      true,
      "a lambda with a block is no expression tree",
    ),
    (
      Grammar::CSharp,
      "class A { int F() { var total = Load(); var count = total.Count; return count; } }\n",
      "class A { int F() { var sum = Load(); var count = sum.Count; return count; } }\n",
      true,
      "a member read of it is no call",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int entry, item, last;\n  void F() {\n    foreach (var entry in xs) { last = entry; }\n    last = entry;\n  }\n}\n",
      "class A {\n  int entry, item, last;\n  void F() {\n    foreach (var item in xs) { last = item; }\n    last = item;\n  }\n}\n",
      false,
      "the statement after the loop keeps another field now",
    ),
    (
      Grammar::CSharp,
      "class A { string F() { var total = 1; return nameof(total); } }\n",
      "class A { string F() { var sum = 1; return nameof(sum); } }\n",
      false,
      "the text of the name is a value",
    ),
    (
      Grammar::CSharp,
      "class A { object F() { var total = 1; return new { total }; } }\n",
      "class A { object F() { var sum = 1; return new { sum }; } }\n",
      false,
      "the member of the anonymous type has another name now",
    ),
    (
      Grammar::CSharp,
      "class A { object F() { var total = 1; return new Box { total = total }; } }\n",
      "class A { object F() { var sum = 1; return new Box { total = sum }; } }\n",
      true,
      "the member the initializer names keeps its name",
    ),
    (
      Grammar::CSharp,
      "var total = 1;\nvar more = total + 1;\nint Twice(int x) => x * total;\n",
      "var sum = 1;\nvar more = sum + 1;\nint Twice(int x) => x * sum;\n",
      true,
      "a local of the top-level statements and its uses in them",
    ),
    (
      Grammar::CSharp,
      "class A { int total = 100; int F(int k) { switch (k) {\n  case 1: int total = 1; k = total; break;\n  case 2: total = 2; return total;\n} return 0; } }\n",
      "class A { int total = 100; int F(int k) { switch (k) {\n  case 1: int sum = 1; k = sum; break;\n  case 2: total = 2; return total;\n} return 0; } }\n",
      false,
      "the other section writes the field now",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int x, y, last;\n  void F() {\n    while (Next() is int x) { last = x; }\n    last = x;\n  }\n}\n",
      "class A {\n  int x, y, last;\n  void F() {\n    while (Next() is int y) { last = y; }\n    last = y;\n  }\n}\n",
      false,
      "the statement after the loop reads another field now",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int x, y, last;\n  void F(bool c) {\n    if (c) last = Next() is int x ? x : 0;\n    last = x;\n  }\n}\n",
      "class A {\n  int x, y, last;\n  void F(bool c) {\n    if (c) last = Next() is int y ? y : 0;\n    last = y;\n  }\n}\n",
      false,
      "the statement after the if reads another field now",
    ),
    (
      Grammar::CSharp,
      "class A {\n  Box item, part;\n  object F() { return from item in item.Parts select item; }\n}\n",
      "class A {\n  Box item, part;\n  object F() { return from part in part.Parts select part; }\n}\n",
      false,
      "the query runs over another field now",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int total, sum;\n  object F(int[] xs) { return from x in xs let total = total + x select total; }\n}\n",
      "class A {\n  int total, sum;\n  object F(int[] xs) { return from x in xs let sum = sum + x select sum; }\n}\n",
      false,
      "the value of the let reads another field now",
    ),
    (
      Grammar::CSharp,
      "class A { string x = \"field\"; string y, last; string F() {\n  lock (Get() is string x ? x : last) { last = x; }\n  return x;\n} }\n",
      "class A { string x = \"field\"; string y, last; string F() {\n  lock (Get() is string y ? y : last) { last = y; }\n  return x;\n} }\n",
      false,
      "the statement after the lock read the variable, and reads the field now",
    ),
    (
      Grammar::CSharp,
      "class A { const int Max = 5; bool F(int x) { const int Max = 1; x += Max; return x is Max; } }\n",
      "class A { const int Max = 5; bool F(int x) { const int Top = 1; x += Top; return x is Max; } }\n",
      false,
      "the pattern reads the constant of the class now",
    ),
    (
      Grammar::CSharp,
      "class A { bool F(int x) { const int Max = 1; return x is Max; } }\n",
      "class A { bool F(int x) { const int Top = 1; return x is Top; } }\n",
      false,
      "a type of the new name, declared anywhere, would take the pattern",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int[] item, part;\n  object F(int[] xs) { return from item in xs join y in item on item equals y select y; }\n}\n",
      "class A {\n  int[] item, part;\n  object F(int[] xs) { return from part in xs join y in part on part equals y select y; }\n}\n",
      false,
      "the source of the join is another field now",
    ),
    (
      Grammar::CSharp,
      "class A {\n  Row s, t;\n  object F(int[] xs) { return from x in xs where Get(out var s) && s.Ok select s; }\n}\n",
      "class A {\n  Row s, t;\n  object F(int[] xs) { return from x in xs where Get(out var t) && t.Ok select t; }\n}\n",
      false,
      "the select reads another field now: the variable of the where ends with it",
    ),
    (
      Grammar::CSharp,
      "class A {\n  Box item, part;\n  object F(int[] xs) { return from x in xs join item in item.Parts on x equals item select item; }\n}\n",
      "class A {\n  Box item, part;\n  object F(int[] xs) { return from x in xs join part in part.Parts on x equals part select part; }\n}\n",
      false,
      "the join runs over another field now",
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
      Grammar::Starlark,
      "def f(c):\n    total = 1\n    if c:\n        total = 2\n    return total\n",
      "def f(c):\n    total = 1\n    if c:\n        count = 2\n    return count\n",
      false,
      "the assignment of the branch was one variable with the first",
    ),
    (
      Grammar::Starlark,
      "total = \"global\"\ndef outer():\n    def inner():\n        return total\n    total = \"local\"\n    return inner()\n",
      "total = \"global\"\ndef outer():\n    def inner():\n        return total\n    count = \"local\"\n    return inner()\n",
      false,
      "the closure reads the name of the file now",
    ),
    (
      Grammar::Starlark,
      "def f(xs, ys):\n    xs = xs + [1]\n    return xs\n",
      "def f(xs, ys):\n    ys = ys + [1]\n    return ys\n",
      false,
      "the assignment writes another parameter now",
    ),
    (
      Grammar::Starlark,
      "def f(a, b):\n    total = a\n    count = b\n    total = total + 1\n    return total\n",
      "def f(a, b):\n    total = a\n    count = b\n    count = count + 1\n    return count\n",
      false,
      "the assignment writes another variable now",
    ),
    (
      Grammar::Starlark,
      "def f(a):\n    if a:\n        total = 1\n    else:\n        total = 2\n    return total\n",
      "def f(a):\n    if a:\n        total = 1\n    else:\n        count = 2\n    return count\n",
      false,
      "the branches assigned one variable",
    ),
    (
      Grammar::Starlark,
      "def f():\n    key = lambda item: item + 1\n    return key(item = 2)\n",
      "def f():\n    key = lambda entry: entry + 1\n    return key(item = 2)\n",
      false,
      "a call names the parameter of the lambda",
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
      Grammar::Rust,
      "fn f() -> S {\n  let total = 1;\n  S { total: total }\n}\n",
      "fn f() -> S {\n  let sum = 1;\n  S { total: sum }\n}\n",
      true,
      "the field keeps its name",
    ),
    (
      Grammar::Rust,
      "static sum: u32 = 7;\nfn f() -> u32 {\n  let total = 1;\n  fn g() -> u32 { sum }\n  g() + total\n}\n",
      "static sum: u32 = 7;\nfn f() -> u32 {\n  let sum = 1;\n  fn g() -> u32 { sum }\n  g() + sum\n}\n",
      true,
      "the fn inside reads the static on both sides: it sees no binding around it",
    ),
    (
      Grammar::Rust,
      "fn f(width: usize, n: u32) -> String {\n  let width = width * 2;\n  format!(\"{n:0width$}\")\n}\n",
      "fn f(width: usize, n: u32) -> String {\n  let pad = width * 2;\n  format!(\"{n:0width$}\")\n}\n",
      false,
      "the format string takes its width from the parameter now",
    ),
    (
      Grammar::Rust,
      "#[tauri::command]\nfn load(total: u32) -> u32 {\n  total + 1\n}\n",
      "#[tauri::command]\nfn load(sum: u32) -> u32 {\n  sum + 1\n}\n",
      false,
      "the attribute macro names the parameter in what it generates",
    ),
    (
      Grammar::Rust,
      "fn f() -> u32 {\n  let total = 1;\n  total::run(total)\n}\n",
      "fn f() -> u32 {\n  let sum = 1;\n  sum::run(sum)\n}\n",
      false,
      "the function of another module is called now",
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
      Grammar::TypeScript,
      "function f() {\n  const total = 1\n  let x: total.T = make()\n  return [total, x]\n}\n",
      "function f() {\n  const sum = 1\n  let x: sum.T = make()\n  return [sum, x]\n}\n",
      false,
      "the type is of another namespace now",
    ),
    (
      Grammar::TypeScript,
      "const outer = (total: number, sum: number) => {\n  const f = (a = total) => { let total = a + 1; return total }\n  return f()\n}\n",
      "const outer = (total: number, sum: number) => {\n  const f = (a = sum) => { let sum = a + 1; return sum }\n  return f()\n}\n",
      false,
      "the default reads another parameter of the function around now",
    ),
    (
      Grammar::TypeScript,
      "function f(xs: number[], ys: number[]) {\n  var xs = xs || []\n  return xs\n}\n",
      "function f(xs: number[], ys: number[]) {\n  var ys = ys || []\n  return ys\n}\n",
      false,
      "the var is the other parameter now",
    ),
    (
      Grammar::TypeScript,
      "function f(a: boolean) {\n  if (a) { var total = 1 } else { var total = 2 }\n  return total\n}\n",
      "function f(a: boolean) {\n  if (a) { var total = 1 } else { var sum = 2 }\n  return sum\n}\n",
      false,
      "the two vars were one variable",
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
    (
      Grammar::JavaScript,
      "function f(c) {\n  var total = 1\n  if (c) { var total = 2 }\n  return total\n}\n",
      "function f(c) {\n  var total = 1\n  if (c) { var sum = 2 }\n  return sum\n}\n",
      false,
      "the var of the block was one variable with the first",
    ),
    (
      Grammar::JavaScript,
      "function f(total) {\n  if (!total) { var total = 1 }\n  return total\n}\n",
      "function f(total) {\n  if (!total) { var sum = 1 }\n  return sum\n}\n",
      false,
      "the var was the parameter",
    ),
    (
      Grammar::JavaScript,
      "function f(a, b) {\n  var total = a\n  var count = b\n  var total = total + 1\n  return total\n}\n",
      "function f(a, b) {\n  var total = a\n  var count = b\n  var count = count + 1\n  return count\n}\n",
      false,
      "the var writes another variable now",
    ),
    (
      Grammar::JavaScript,
      "function outer(total, sum) {\n  function f(a = total) { let total = a + 1; return total }\n  return f()\n}\n",
      "function outer(total, sum) {\n  function f(a = sum) { let sum = a + 1; return sum }\n  return f()\n}\n",
      false,
      "the default reads another parameter of the function around now",
    ),
    // the name is a value where the code runs
    (
      Grammar::Kotlin,
      "fun f(m: Map<String, Int>): Int {\n  val total by m\n  return total\n}\n",
      "fun f(m: Map<String, Int>): Int {\n  val sum by m\n  return sum\n}\n",
      false,
      "the delegate reads the key of the new name",
    ),
    (
      Grammar::KotlinScript,
      "tasks {\n  val integrationTest by registering(Test::class)\n  check { dependsOn(integrationTest) }\n}\n",
      "tasks {\n  val integTest by registering(Test::class)\n  check { dependsOn(integTest) }\n}\n",
      false,
      "the task is registered under the new name",
    ),
    (
      Grammar::Kotlin,
      "class A {\n  init {\n    val total by lazy { 1 }\n    use(total)\n  }\n}\n",
      "class A {\n  init {\n    val sum by lazy { 1 }\n    use(sum)\n  }\n}\n",
      false,
      "the delegate gets the new name",
    ),
    (
      Grammar::TypeScript,
      "function f() {\n  const total = () => 1\n  return total\n}\n",
      "function f() {\n  const sum = () => 1\n  return sum\n}\n",
      false,
      "the function takes the new name as its own",
    ),
    (
      Grammar::JavaScript,
      "function f() {\n  const Total = class {}\n  return Total\n}\n",
      "function f() {\n  const Sum = class {}\n  return Sum\n}\n",
      false,
      "the class takes the new name as its own",
    ),
    // the scopes read a part of the code
    (
      Grammar::CSharp,
      "class A {\n  int total = 5;\n  int F() {\n    int total = 1;\n#if DEBUG\n    Log(total);\n#endif\n    return total;\n  }\n}\n",
      "class A {\n  int total = 5;\n  int F() {\n    int sum = 1;\n#if DEBUG\n    Log(total);\n#endif\n    return sum;\n  }\n}\n",
      false,
      "the branch not taken logs the field now",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int sum = 5;\n  int F() {\n    int total = 1;\n#if NET8_0_OR_GREATER\n    Fast(sum);\n#else\n    Slow(sum);\n#endif\n    return total;\n  }\n}\n",
      "class A {\n  int sum = 5;\n  int F() {\n    int sum = 1;\n#if NET8_0_OR_GREATER\n    Fast(sum);\n#else\n    Slow(sum);\n#endif\n    return sum;\n  }\n}\n",
      false,
      "both branches read the local now, and one of them is no code here",
    ),
    (
      Grammar::CSharp,
      "class A {\n  int total = 5, sum = 6;\n  int F() {\n#if !DEBUG\n    int total = 1;\n#endif\n    return total;\n  }\n}\n",
      "class A {\n  int total = 5, sum = 6;\n  int F() {\n#if !DEBUG\n    int sum = 1;\n#endif\n    return sum;\n  }\n}\n",
      false,
      "where the branch is not compiled, the method returns another field now",
    ),
    (
      Grammar::Rust,
      "fn f(sum: u32) -> u32 {\n  #[cfg(unix)]\n  let total = 1;\n  #[cfg(windows)]\n  let total = 2;\n  total\n}\n",
      "fn f(sum: u32) -> u32 {\n  #[cfg(unix)]\n  let total = 1;\n  #[cfg(windows)]\n  let sum = 2;\n  sum\n}\n",
      false,
      "where the first condition holds, the function returns its parameter now",
    ),
    (
      Grammar::JavaScript,
      "function f(code) {\n  const total = 1\n  keep(total)\n  return eval(code)\n}\n",
      "function f(code) {\n  const sum = 1\n  keep(sum)\n  return eval(code)\n}\n",
      false,
      "the text the eval runs may name the variable",
    ),
    (
      Grammar::JavaScript,
      "function f(o) {\n  const total = 1\n  with (o) { use(total) }\n}\n",
      "function f(o) {\n  const sum = 1\n  with (o) { use(sum) }\n}\n",
      false,
      "in the with, the new name may be a property of the object",
    ),
    (
      Grammar::Java,
      "class A {\n  int total = 5;\n  int f() {\n    int total = 1;\n    x = ) total (;\n    return total;\n  }\n}\n",
      "class A {\n  int total = 5;\n  int f() {\n    int sum = 1;\n    x = ) total (;\n    return sum;\n  }\n}\n",
      false,
      "what the parser gave up on still names the variable",
    ),
    (
      Grammar::Kotlin,
      "fun f(): Int {\n  val total = 1\n  return total\n}\n\nfun broken( {\n",
      "fun f(): Int {\n  val sum = 1\n  return sum\n}\n\nfun broken( {\n",
      false,
      "a text with an error is read in part",
    ),
  ];
  // every case is read, so that a change of the scopes shows all it moves
  let mut wrong = Vec::new();
  for &(grammar, old, new, expected, why) in cases {
    let (old_name, new_name) = words(old).iter().zip(&words(new)).find(|it| it.0.1 != it.1.1).map(|it| (it.0.1, it.1.1)).unwrap();
    if is_rename(grammar, old, new, old_name, new_name) != expected {
      wrong.push(format!("{grammar:?}, expected {expected}: {why}\n{old}\n{new}"));
    }
  }
  assert!(wrong.is_empty(), "{} of {} cases read wrong:\n{}", wrong.len(), cases.len(), wrong.join("\n"));
}

/// The lines of the statements of the `occurrence`-th `needle` of a text asked with another name, as `from..to`
/// each (from 0, the end excluded)
fn statements_at(grammar: Grammar, text: &str, needle: &str, occurrence: usize) -> String {
  let question = NameQuestion {
    position: place(text, needle, occurrence),
    renamed_to: Some("other"),
  };
  let found = names_at(grammar, text, &[question]);
  let rows = found.of(0).and_then(|it| it.statements.clone()).unwrap_or_default();
  rows.iter().map(|row| format!("{}..{}", row.start, row.end)).collect::<Vec<_>>().join(" ")
}

/// The statements of a local are the member around it, whole: the function of a class or of a file with its head,
/// whatever is nested between the two (a lambda, a function of its own, a class declared in it), the property a
/// lambda is the value of, never the class or the namespace that holds the member. What gives a local its value
/// stands anywhere in that code. Those of any other name are the statements of its declarations, each with its
/// bodies but the members of a type
#[test]
fn the_statements_of_a_local_are_the_member_around_it_and_those_of_another_name_its_declarations() {
  let kotlin = "package demo\n\nclass Ledger(val store: Store) {\n  val total = 1\n\n  /** Sends. */\n  fun send(users: List<User>) {\n    users\n      .filter { it.active }\n      .forEach { user ->\n        fun inner(count: Int) = log(count)\n        inner(user.id)\n      }\n  }\n\n  val handler = { event: Event ->\n    log(event)\n  }\n\n  init {\n    val first = 1\n    log(first)\n  }\n}\n\nfun top(scale: Int): Int {\n  val helper = object : Runnable {\n    override fun run() {\n      val inside = scale\n      log(inside)\n    }\n  }\n  return scale\n}\n";
  let java = "class A {\n  static final int LIMIT =\n      5;\n\n  int pause(\n      int time) {\n    Runnable later = new Runnable() {\n      public void run() {\n        int inside = time;\n        log(inside);\n      }\n    };\n    return time;\n  }\n\n  enum Mode {\n    FAST {\n      int cost(int size) {\n        return size;\n      }\n    }\n  }\n}\n";
  let csharp = "namespace Demo {\n  class A {\n    int F(int[] items) {\n      var total = 0;\n      foreach (var item in items) {\n        total += item;\n      }\n      return total;\n    }\n  }\n}\n";
  let starlark = "LIMIT = 5\n\ndef f(items):\n    total = 0\n    for item in items:\n        total += item\n    return total\n";
  let rust = "mod inner {\n  impl Ledger {\n    /// Sums.\n    fn sum(&self, items: &[u32]) -> u32 {\n      let total = items.iter().map(|item| item + 1).sum();\n      fn twice(count: u32) -> u32 {\n        count * 2\n      }\n      twice(total)\n    }\n  }\n}\n\nconst LIMIT: u32 =\n  5;\n";
  let typescript = "export class Ledger {\n  total = 0\n\n  sum(items: number[]): number {\n    return items.reduce((acc, item) => {\n      return acc + item\n    }, 0)\n  }\n}\n\nexport function top(scale: number): number {\n  const twice = (count: number) => count * 2\n  return twice(scale)\n}\n\nexport const LIMIT =\n  5\n\nitems.forEach(entry => {\n  log(entry)\n})\n";
  let rows: &[(Grammar, &str, &str, usize, &str)] = &[
    // the parameter of a lambda, of a function declared in it and a parameter of the method: the method, from its
    // name to its last line, without its doc comment
    (Grammar::Kotlin, kotlin, "user ->", 0, "6..14"),
    (Grammar::Kotlin, kotlin, "count:", 0, "6..14"),
    (Grammar::Kotlin, kotlin, "users:", 0, "6..14"),
    // the parameter of a lambda that is the value of a property: the property; a local of an initializer: the `init`
    (Grammar::Kotlin, kotlin, "event:", 0, "15..18"),
    (Grammar::Kotlin, kotlin, "first =", 0, "19..23"),
    // a local of a method of an object declared in a function is of that function, whose variables it reads
    (Grammar::Kotlin, kotlin, "inside =", 0, "25..34"),
    (Grammar::Kotlin, kotlin, "scale:", 0, "25..34"),
    // a name that is not local: its declaration alone, a function with its block, a class without its members
    (Grammar::Kotlin, kotlin, "total =", 0, "3..4"),
    (Grammar::Kotlin, kotlin, "send", 0, "6..14"),
    (Grammar::Kotlin, kotlin, "Ledger", 0, "2..3"),
    (Grammar::Kotlin, kotlin, "store:", 0, "2..3"),
    // Java: a local of a method of an anonymous class is of the method around it; a method of a constant of an enum
    (Grammar::Java, java, "inside =", 0, "4..14"),
    (Grammar::Java, java, "time)", 0, "4..14"),
    (Grammar::Java, java, "size)", 0, "17..20"),
    (Grammar::Java, java, "LIMIT", 0, "1..3"),
    (Grammar::Java, java, "pause", 0, "4..14"),
    // C#: a method of a class of a namespace
    (Grammar::CSharp, csharp, "total =", 0, "2..9"),
    (Grammar::CSharp, csharp, "item in", 0, "2..9"),
    // Starlark: a `def`; a name of the file is no local, and its statement is its own
    (Grammar::Starlark, starlark, "total =", 0, "2..7"),
    (Grammar::Starlark, starlark, "LIMIT", 0, "0..1"),
    // Rust: a function of an `impl` of a module, with a function declared in it; a constant on two lines
    (Grammar::Rust, rust, "total =", 0, "3..10"),
    (Grammar::Rust, rust, "item|", 0, "3..10"),
    (Grammar::Rust, rust, "count:", 0, "3..10"),
    (Grammar::Rust, rust, "LIMIT", 0, "13..15"),
    // TypeScript: a method of an exported class, an exported function, a statement of the module
    (Grammar::TypeScript, typescript, "acc,", 0, "3..8"),
    (Grammar::TypeScript, typescript, "count:", 0, "10..14"),
    (Grammar::TypeScript, typescript, "scale:", 0, "10..14"),
    (Grammar::TypeScript, typescript, "entry =>", 0, "18..21"),
    (Grammar::TypeScript, typescript, "LIMIT", 0, "15..17"),
    (Grammar::TypeScript, typescript, "Ledger", 0, "0..1"),
  ];
  let found: Vec<_> = rows
    .iter()
    .map(|&(grammar, _, needle, occurrence, _)| {
      (
        grammar,
        needle,
        statements_at(grammar, rows.iter().find(|row| row.0 == grammar && row.2 == needle).unwrap().1, needle, occurrence),
      )
    })
    .collect();
  let expected: Vec<_> = rows.iter().map(|&(grammar, _, needle, _, lines)| (grammar, needle, lines.to_string())).collect();
  assert_eq!(found, expected);
}
