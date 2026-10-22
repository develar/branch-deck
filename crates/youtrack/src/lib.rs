//! YouTrack as far as Branch Deck uses it: the app's own sign-in through the Hub of a YouTrack, the issues of the
//! popovers, and the command that marks an issue fixed.

pub mod client;
#[cfg(any(test, feature = "fake"))]
pub mod fake_youtrack;
pub mod issue;
pub mod service;
pub mod sign_in;
pub mod sign_in_session;
pub mod token_store;
pub mod url;

#[cfg(test)]
mod issue_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod sign_in_session_test;
#[cfg(test)]
mod sign_in_test;
#[cfg(test)]
mod url_test;
