//! What `bd suggest` and `bd commits` compute from the commits ahead of the baseline, without the CLI, so that a change
//! of the clustering rebuilds this crate and not the commands:
//! - `clustering`: the commits clustered into branches, the pure core of `bd suggest`
//! - `areas`: the directories a commit changes, cut to a fixed depth
//! - `hub_files`: the registries and docs that the areas of a commit leave out (the generated files: `generated-files`)

pub mod areas;
pub mod clustering;
pub mod hub_files;

#[cfg(test)]
mod areas_test;
#[cfg(test)]
mod clustering_test;
