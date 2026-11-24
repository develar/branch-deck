//! The `CodeOwners` service of the IntelliJ monorepo: who the members and the reviewers of a group of code owners are,
//! with their GitHub logins and whether they are available (`client`). HTTP only, asked with the Space token Patronus
//! takes (`patronus::token`, found by the caller); the owner groups of the changed paths come from the `OWNERSHIP` files
//! (`git_ops::ownership`), whom a review after landing asks of them is `domain_policy::shown::show_candidates`.

pub mod client;
#[cfg(any(test, feature = "fake"))]
pub mod fake_codeowners;

#[cfg(test)]
mod client_test;
