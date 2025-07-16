#!/usr/bin/env -S cargo +nightly -Zscript
//! ```cargo
//! [package]
//! edition = "2021"
//! 
//! [dependencies]
//! test-utils = { path = "../../crates/test-utils" }
//! ```

use std::path::Path;
use test_utils::templates;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test_repos_dir = Path::new("tests/test-repos");
    
    println!("Regenerating test repository templates...");
    
    // Remove existing templates
    if test_repos_dir.exists() {
        std::fs::remove_dir_all(test_repos_dir)?;
    }
    
    // Create directory
    std::fs::create_dir_all(test_repos_dir)?;
    
    // Create templates
    let templates_to_create = vec![
        ("simple", templates::simple()),
        ("issue-reference", templates::issue_reference()),
        ("unassigned", templates::unassigned()),
    ];
    
    for (name, template) in templates_to_create {
        let repo_path = test_repos_dir.join(name);
        println!("Creating template: {}", name);
        template.build(&repo_path)?;
    }
    
    println!("Test repository templates regenerated successfully!");
    Ok(())
}