//! Hub files: the registries and docs that changes in many unrelated areas touch, which the areas of a commit leave
//! out

/// Files that changes in many unrelated areas touch: module and build registries, file lists and docs. They tell
/// nothing about where a commit belongs, so the areas leave them out. A pattern matches the file name, with `*` at its
/// start or end for any text; a pattern with a `/` matches the end of the path
const HUB_FILES: [&str; 5] = [".idea/modules.xml", "MODULE.bazel", "*-file-list.txt", "README*", "CHANGELOG*"];

pub fn is_hub_file(path: &str) -> bool {
  let name = file_name(path);
  HUB_FILES.iter().any(|pattern| {
    if let Some(suffix) = pattern.strip_prefix('*') {
      name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
      name.starts_with(prefix)
    } else if pattern.contains('/') {
      path.strip_suffix(pattern).is_some_and(|rest| rest.is_empty() || rest.ends_with('/'))
    } else {
      name == *pattern
    }
  })
}

fn file_name(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}
