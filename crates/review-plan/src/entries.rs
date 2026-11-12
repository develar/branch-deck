//! The entry points of a module a change adds (openspec/specs/branch/review-plan/spec.md, "Modules"): its declarations something
//! outside it uses, from accio (`accio surface`, one process for every new module, at the head, within a budget): a
//! class a plugin descriptor registers first, then the declarations the code of other modules uses, a use by the name
//! alone or from test code entering nothing. The opening of the group of the module names them and its stops start with
//! their files (`groups::order_by_entries`). Without an answer (no accio, an accio without `surface`) no module knows
//! them, which is no reason of `ReviewPlan::index_note`: the uses and the tests are known all the same

use crate::fan_in::ask_index;
use crate::modules::{ModuleShape, ModuleState};
use anyhow::Result;
use file_diffs::accio::{SurfaceDeclaration, SymbolIndex};
use serde::Serialize;

/// What enters a module the change adds (`ModuleShape::entries`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ModuleEntries {
  /// accio asked every declaration of the module within its budget and tied every use it found: `points` is all that
  /// enters it
  pub complete: bool,
  /// accio's answer is of the load of the moment, not of the trees (`ModuleSurface::cut`): its `--budget` seconds ran
  /// out (`"time"`, or a cut this version does not know), so asked again it may say more and a plan that holds it is
  /// not kept (`ReviewPlan::is_final`). A bound on files read (`"work"`) cuts the same on every run and a use left
  /// unresolved alone cuts nothing (`null`): neither is a cut. An accio before the field: `complete` false with a note
  /// (its budget ran out, its index could not answer)
  #[serde(skip)]
  pub cut: bool,
  /// A class a descriptor registers first, then the declarations other code uses, as accio ranks them
  pub points: Vec<EntryPoint>,
}

/// A declaration of a new module something outside it uses
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct EntryPoint {
  pub name: String,
  /// Its qualified name; `None` where the language qualifies none
  pub qualified: Option<String>,
  /// accio's kind: `class`, `function`, …
  pub kind: String,
  /// Its file at the head and the line of its name, from 1
  pub path: String,
  pub line: u32,
  /// A plugin descriptor registers it: the extension point and what it is registered for (the interface of a service,
  /// the topic of a listener), of its first registration
  pub registered: Option<EntryRegistration>,
  /// The files of the code outside the module that use it, each at its first use, by path: a descriptor's own file
  /// aside
  pub used_by: Vec<EntrySite>,
}

/// How a descriptor registers an entry point
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct EntryRegistration {
  /// `com.intellij.applicationService`
  pub point: String,
  /// What it is registered for: the interface of a service, the topic of a listener; `None` for none
  pub of: Option<String>,
  /// The descriptor and the line of the registration, from 1
  pub path: String,
  pub line: u32,
}

/// A place of a use
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct EntrySite {
  pub path: String,
  /// From 1
  pub line: u32,
}

/// `ModuleShape::entries` of each module the change adds at `head`, in one query of the index; none without an answer
pub(crate) fn add_entries(index: Option<&dyn SymbolIndex>, repository_path: &str, head: &str, modules: &mut [ModuleShape]) -> Result<()> {
  let asked: Vec<String> = modules
    .iter()
    .filter(|module| module.state == ModuleState::Added && !module.root.is_empty())
    .map(|module| module.root.clone())
    .collect();
  if asked.is_empty() {
    return Ok(());
  }
  let Ok(surfaces) = ask_index(index, |lenient| lenient.surface(repository_path, &asked, head))? else {
    return Ok(());
  };
  for surface in surfaces {
    let Some(module) = modules.iter_mut().find(|module| module.root == surface.module) else {
      continue;
    };
    module.entries = Some(ModuleEntries {
      complete: surface.complete,
      cut: match surface.cut.as_ref() {
        Some(Some(cut)) => cut != "work",
        Some(None) => false,
        None => !surface.complete && surface.note.is_some(),
      },
      points: surface.declarations.iter().filter_map(entry_point).collect(),
    });
  }
  Ok(())
}

/// The entry point a declaration is: of a registration or of a use resolved from the code outside the module, not by
/// the name alone nor from test code
fn entry_point(declaration: &SurfaceDeclaration) -> Option<EntryPoint> {
  let resolved = |resolved: &Option<String>| resolved.as_deref() != Some("name");
  let registered = declaration
    .uses
    .iter()
    .find(|it| it.label == "descriptor" && resolved(&it.resolved))
    .map(|it| EntryRegistration {
      point: it.point.clone().unwrap_or_default(),
      of: it.of.clone(),
      path: it.file.clone(),
      line: it.line,
    });
  let entered = registered.is_some() || declaration.uses.iter().any(|it| it.label != "descriptor" && !it.test && resolved(&it.resolved));
  if !entered {
    return None;
  }
  let descriptors: Vec<&str> = declaration.uses.iter().filter(|it| it.label == "descriptor").map(|it| it.file.as_str()).collect();
  Some(EntryPoint {
    name: declaration.name.clone(),
    qualified: declaration.qualified.clone(),
    kind: declaration.kind.clone(),
    path: declaration.file.clone(),
    line: declaration.line,
    registered,
    used_by: declaration
      .used_by
      .iter()
      .filter(|site| !descriptors.contains(&site.file.as_str()))
      .map(|site| EntrySite {
        path: site.file.clone(),
        line: site.line,
      })
      .collect(),
  })
}
