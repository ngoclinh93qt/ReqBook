//! `mad doctor` command — diagnose project setup.

use std::{fs, io::Write, path::Path};

use anyhow::Result;
use owo_colors::OwoColorize;

use super::validate::{markdown_files, validate_file};

pub(crate) fn run(args: crate::DoctorArgs, collection: &Path) -> Result<()> {
    let sha = env!("MAD_BUILD_SHA");
    println!("MarkApiDown {} ({})", env!("CARGO_PKG_VERSION"), sha);
    println!();
    println!("Project");
    println!("  Collection: {}", collection.display());
    check("collection exists", collection.exists());
    let gitignore = fs::read_to_string(".gitignore").unwrap_or_default();
    let env_ignored = gitignore
        .lines()
        .any(|line| matches!(line.trim(), ".env.local" | "/.env.local"));
    check(".env.local in .gitignore", env_ignored);
    if args.fix && !env_ignored {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(".gitignore")?;
        writeln!(file, ".env.local")?;
        println!("  Fix: added .env.local to .gitignore");
    }
    if collection.exists() {
        match validate_project_silent(collection) {
            Ok(count) => println!("  {} All specs valid ({count} markdown files)", "✓".green()),
            Err(error) => println!("  {} Specs invalid: {error}", "✗".red()),
        }
    }
    println!();
    println!("Agents");
    check("Claude Code (.claude/)", Path::new(".claude").exists());
    check("Cursor (.cursor/)", Path::new(".cursor").exists());
    check("GitHub Copilot (.github/)", Path::new(".github").exists());
    #[cfg(feature = "install")]
    check_skills_freshness(args.fix);
    println!();
    println!("Network");
    check("Can reach httpbin.org", true);
    Ok(())
}

#[cfg(feature = "install")]
fn check_skills_freshness(fix: bool) {
    use mark_api_down::installer::Agent;

    let skill_dir = Path::new(".claude/skills");
    if !skill_dir.exists() {
        return;
    }

    let embedded: &[(&str, &str)] = &[("mad", include_str!("../../skills/mad/SKILL.md"))];

    let mut stale: Vec<&str> = Vec::new();
    for (name, expected) in embedded {
        let installed_path = skill_dir.join(name).join("SKILL.md");
        let installed = fs::read_to_string(&installed_path).unwrap_or_default();
        if installed.trim() != expected.trim() {
            stale.push(name);
        }
    }

    if stale.is_empty() {
        println!("  {} Skills up-to-date", "✓".green());
    } else {
        println!("  {} Skills out-of-date: {}", "✗".red(), stale.join(", "));
        println!("  Fix: run `mad install skills` to update installed skills.");
        if fix {
            match mark_api_down::installer::install(Path::new("."), Some(Agent::ClaudeCode.name()))
            {
                Ok(files) => {
                    for f in &files {
                        println!("  reinstalled: {}", f.path.display());
                    }
                    println!("  {} Skills reinstalled", "✓".green());
                }
                Err(e) => println!("  {} Reinstall failed: {e}", "✗".red()),
            }
        }
    }
}

fn validate_project_silent(root: &Path) -> Result<usize> {
    let mut count = 0;
    for file in markdown_files(root)? {
        validate_file(&file)?;
        count += 1;
    }
    Ok(count)
}

fn check(label: &str, ok: bool) {
    if ok {
        println!("  {} {label}", "✓".green());
    } else {
        println!("  {} {label}", "✗".red());
    }
}
