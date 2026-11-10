use crate::api::shown_constructors;
use crate::shown::{FormMark, ShownForm, mark_changes};
use code_symbols::language::Language;
use code_symbols::summary::Reach;
use pretty_assertions::assert_eq;
use trial_check::symbol_dependencies::declaration_checks::{ConstructorForm, Form, ParameterForm, ShownType};

/// A parameter: `name:type_text` with `=` for a default, `*` for a vararg, and its type as written after `@`:
/// `dirs:List/Path@List<Path>`
fn parameter(text: &str) -> ParameterForm {
  let (text, shown) = text.split_once('@').map_or((text, None), |(text, shown)| (text, Some(shown.to_string())));
  let (text, vararg) = text.strip_suffix('*').map_or((text, false), |text| (text, true));
  let (text, optional) = text.strip_suffix('=').map_or((text, false), |text| (text, true));
  let (name, type_text) = text.split_once(':').unwrap_or((text, ""));
  ParameterForm {
    name: name.to_string(),
    type_text: type_text.to_string(),
    shown_type: ShownType(shown),
    optional,
    vararg,
  }
}

fn constructor(reach: Reach, parameters: &[&str]) -> ConstructorForm {
  ConstructorForm {
    reach,
    parameters: parameters.iter().map(|it| parameter(it)).collect(),
  }
}

/// The form of a type of `constructors`
fn type_form(constructors: Vec<ConstructorForm>) -> Form {
  Form {
    header: None,
    parameters: None,
    reach: Reach::Public,
    overridable: false,
    jvm: String::new(),
    constructors: Some(constructors),
  }
}

/// The forms of a type `name` before (`old`) and after (`new`) a change, as `language` writes them, with what the change did
/// marked
fn changed(name: &str, old: &Form, new: &Form, language: Option<Language>) -> (ShownForm, ShownForm) {
  let shown = |form: &Form| shown_constructors(name, form.constructors.as_deref().unwrap_or_default(), Reach::Public, language, false);
  let (mut before, mut after) = (shown(old), shown(new));
  mark_changes(&mut before, old, &mut after, new, language);
  (before, after)
}

/// The marked parts of each line: a whole line `-line`/`+line`, a parameter by its text and mark
fn marks(form: &ShownForm) -> Vec<String> {
  form
    .lines
    .iter()
    .flat_map(|line| {
      let whole = line.mark.map(|mark| format!("{mark:?} line {}", line.text()));
      whole.into_iter().chain(
        line
          .parameters
          .iter()
          .filter_map(|parameter| parameter.mark.map(|mark| format!("{mark:?} {}", parameter.text()))),
      )
    })
    .collect()
}

/// The constructors of a type read as calls of it, a line each, each after the reach a call of it has (the narrower of
/// its own and its type's) as the language writes it: Kotlin `private` and `internal`, Java's package none and `public`
/// only where a reach changed; a parameter by its name and its type as written, a default as `= …`, Kotlin's vararg
/// after `vararg`, Java's type before its name; a type of several names without one written (C#) no type at all,
/// never `List/Window`; never the name of a `Reach`
#[test]
fn a_form_of_constructors_and_parameters_reads_as_code() {
  let (kotlin, java) = (Some(Language::Kotlin), Some(Language::Java));
  let event = [
    constructor(
      Reach::Public,
      &[
        "text:String@String",
        "styles:List/StyleRange@List<StyleRange>",
        "start:Long@Long",
        "ends:List/Long=@List<Long>?",
        "tags:String[]*@String",
      ],
    ),
    constructor(Reach::Container, &[]),
  ];
  assert_eq!(
    shown_constructors("Event", &event, Reach::Public, kotlin, false).text(),
    "Event(text: String, styles: List<StyleRange>, start: Long, ends: List<Long>? = …, vararg tags: String)\nprivate Event()"
  );
  let port = |reach: Reach| [constructor(reach, &["value:Int?@Int?"])];
  assert_eq!(
    shown_constructors("Port", &port(Reach::Module), Reach::Public, kotlin, false).text(),
    "internal Port(value: Int?)"
  );
  // an internal type: its public constructor is called only in its module
  assert_eq!(
    shown_constructors("Port", &port(Reach::Public), Reach::Module, kotlin, false).text(),
    "internal Port(value: Int?)"
  );
  let ports = [
    constructor(Reach::Module, &[]),
    constructor(Reach::Public, &["value:int@int", "names:String[]*@String...", "grid:int[][]@int[][]"]),
  ];
  assert_eq!(
    shown_constructors("Port", &ports, Reach::Public, java, false).text(),
    "Port()\nPort(int value, String... names, int[][] grid)"
  );
  assert_eq!(
    shown_constructors("Port", &ports, Reach::Public, java, true).text(),
    "Port()\npublic Port(int value, String... names, int[][] grid)"
  );
  // a summary without a written type: one name shows, several do not
  assert_eq!(
    shown_constructors("Port", &[constructor(Reach::Public, &["value:int", "handler:String/Int/Unit"])], Reach::Public, None, false).text(),
    "Port(value: int, handler)"
  );
}

/// What a change did to each parameter of a constructor, a Kotlin one paired by its name: a new one added, a gone one
/// removed, one of another type or vararg changed on both sides, one whose type is only written in other words, or
/// moved, not marked
#[test]
fn a_kotlin_parameter_pairs_by_its_name() {
  let old = type_form(vec![constructor(
    Reach::Public,
    &[
      "classPath:ClassPath@ClassPath",
      "parents:Array/PluginModuleDescriptor@Array<PluginModuleDescriptor>",
      "prefix:String?@String?",
      "libDirectories:List/Path@List<Path>",
      "loader:ClassLoader@ClassLoader",
    ],
  )]);
  let new = type_form(vec![constructor(
    Reach::Public,
    &[
      "classPath:ClassPath@ClassPath",
      "prefix:String@String",
      "parents:Array/PluginModuleDescriptor@Array< PluginModuleDescriptor >",
      "libDirectories:List/Path@List<Path>",
      "jnaModule:PluginModuleDescriptor?@PluginModuleDescriptor?",
    ],
  )]);
  let (before, after) = changed("PluginClassLoader", &old, &new, Some(Language::Kotlin));
  assert_eq!(marks(&before), ["TypeChanged prefix: String?", "Removed loader: ClassLoader"]);
  assert_eq!(marks(&after), ["TypeChanged prefix: String", "Added jnaModule: PluginModuleDescriptor?"]);
  // the marked type alone: what the card highlights of a changed one
  assert_eq!(after.lines[0].parameters[1].shown_type, "String");
}

/// A Java parameter pairs by its place, as a call binds it: the longest run of the same types stays (a renamed one too),
/// those between pair by their order as a change of type, the rest of the longer side added or removed
#[test]
fn a_java_parameter_pairs_by_its_place() {
  let java = Some(Language::Java);
  let check = |old: &[&str], new: &[&str]| {
    let (before, after) = changed(
      "Port",
      &type_form(vec![constructor(Reach::Public, old)]),
      &type_form(vec![constructor(Reach::Public, new)]),
      java,
    );
    (marks(&before), marks(&after))
  };
  assert_eq!(
    check(&["a:int@int", "b:String@String"], &["a:int@int", "x:long@long", "renamed:String@String"]),
    (vec![], vec!["Added long x".to_string()]),
    "an insertion in the middle; a rename is no change to a call"
  );
  assert_eq!(
    check(&["a:int@int", "b:String@String", "c:int@int"], &["a:int@int", "b:Path@Path", "c:int@int"]),
    (vec!["TypeChanged String b".to_string()], vec!["TypeChanged Path b".to_string()])
  );
  assert_eq!(
    check(&["a:int@int", "rest:String[]*@String..."], &["a:int@int", "rest:String[]@String[]"]),
    (vec!["TypeChanged String... rest".to_string()], vec!["TypeChanged String[] rest".to_string()]),
    "no longer a vararg"
  );
}

/// The overloads of a type pair first where they are the same, then by the most parameters they keep; one left without a
/// pair is marked whole, its parameters not
#[test]
fn an_overload_pairs_by_the_most_parameters_kept_and_one_left_is_marked_whole() {
  let kotlin = Some(Language::Kotlin);
  let old = type_form(vec![
    constructor(Reach::Public, &[]),
    constructor(Reach::Public, &["name:String@String", "size:Int@Int"]),
    constructor(Reach::Public, &["path:Path@Path"]),
  ]);
  let new = type_form(vec![
    constructor(Reach::Public, &[]),
    constructor(Reach::Public, &["id:Long@Long"]),
    constructor(Reach::Public, &["name:String@String", "size:Long@Long", "flag:Boolean@Boolean"]),
  ]);
  let (before, after) = changed("Item", &old, &new, kotlin);
  assert_eq!(marks(&before), ["TypeChanged size: Int", "Removed line Item(path: Path)"]);
  assert_eq!(marks(&after), ["Added line Item(id: Long)", "TypeChanged size: Long", "Added flag: Boolean"]);
  assert_eq!(before.lines[2].parameters[0].mark, None, "a line marked whole marks no parameter");
  assert_eq!(before.lines[2].mark, Some(FormMark::Removed));
}

/// The only constructor of each side pairs whatever it keeps: a list changed whole is a change of each parameter
#[test]
fn the_only_constructor_of_each_side_pairs_whatever_it_keeps() {
  let kotlin = Some(Language::Kotlin);
  let old = type_form(vec![constructor(Reach::Public, &["path:Path@Path"])]);
  let new = type_form(vec![constructor(Reach::Container, &["id:Long@Long"])]);
  let (before, after) = changed("Item", &old, &new, kotlin);
  assert_eq!(
    (marks(&before), marks(&after)),
    (vec!["Removed path: Path".to_string()], vec!["Added id: Long".to_string()])
  );
  assert_eq!(after.text(), "private Item(id: Long)");
}
