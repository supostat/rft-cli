use owo_colors::OwoColorize;

use crate::context::build_context;
use crate::git::promote::{
    ChangeSource, find_conflicts, get_changed_files, get_local_dirty_files,
    promote_files,
};
use crate::git::get_worktree_by_index;

pub async fn run(index: usize, dry_run: bool, file_filter: Option<&str>) -> miette::Result<()> {
    run_inner(index, dry_run, file_filter)
        .await
        .map_err(miette::Report::new)
}

async fn run_inner(
    index: usize,
    dry_run: bool,
    file_filter: Option<&str>,
) -> crate::error::Result<()> {
    let context = build_context().await?;
    let worktree = get_worktree_by_index(&context.repo_root, index).await?;

    let current_branch_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&context.repo_root)
        .output()
        .await?;

    if !current_branch_output.status.success() {
        return Err(crate::error::RftError::CommandFailed {
            cmd: "git rev-parse --abbrev-ref HEAD".to_string(),
            stderr: String::from_utf8_lossy(&current_branch_output.stderr).to_string(),
        });
    }

    let main_branch = String::from_utf8_lossy(&current_branch_output.stdout)
        .trim()
        .to_string();

    let mut changed_files = get_changed_files(
        &worktree.path,
        &worktree.branch,
        &main_branch,
    )
    .await?;

    if let Some(pattern) = file_filter {
        changed_files.retain(|file| glob_matches(pattern, &file.path));
    }

    if changed_files.is_empty() {
        println!("{}", "No changed files to promote.".dimmed());
        return Ok(());
    }

    let dirty_files = get_local_dirty_files(&context.repo_root).await?;
    let conflicts = find_conflicts(&changed_files, &dirty_files);

    if !conflicts.is_empty() {
        eprintln!(
            "{} The following files have local changes that would be overwritten:",
            "Conflict!".red().bold()
        );
        for conflict in &conflicts {
            eprintln!("  {} {}", "!".red(), conflict);
        }
        return Err(crate::error::RftError::PromoteConflict {
            reason: "conflicting local changes".to_string(),
        });
    }

    if dry_run {
        println!(
            "{} from {} ({}):",
            "Would promote".cyan().bold(),
            format!("[{}]", worktree.index).bold(),
            worktree.branch.bold()
        );
        for file in &changed_files {
            let source_label = match file.source {
                ChangeSource::Committed => "committed".green().to_string(),
                ChangeSource::Uncommitted => "uncommitted".yellow().to_string(),
                ChangeSource::Untracked => "untracked".dimmed().to_string(),
            };
            println!("  {} {} ({})", "+".green(), file.path, source_label);
        }
        println!(
            "\n{} {} file(s)",
            "Total:".bold(),
            changed_files.len()
        );
        return Ok(());
    }

    println!(
        "{} from {} ({})...",
        "Promoting".green().bold(),
        format!("[{}]", worktree.index).bold(),
        worktree.branch.bold()
    );

    let result = promote_files(
        &context.repo_root,
        &worktree.path,
        &worktree.branch,
        &changed_files,
    )
    .await?;

    println!(
        "{} {} file(s): {} via git checkout, {} via file copy",
        "Promoted".green().bold(),
        result.files.len(),
        result.git_checkout_count,
        result.file_copy_count,
    );

    for promoted in &result.files {
        println!("  {} {}", "+".green(), promoted.path);
    }

    Ok(())
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    glob_matches_recursive(pattern.as_bytes(), path.as_bytes())
}

fn glob_matches_recursive(pattern: &[u8], text: &[u8]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            glob_matches_recursive(&pattern[1..], text)
                || (!text.is_empty() && glob_matches_recursive(pattern, &text[1..]))
        }
        (Some(b'?'), Some(_)) => glob_matches_recursive(&pattern[1..], &text[1..]),
        (Some(a), Some(b)) if a == b => glob_matches_recursive(&pattern[1..], &text[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_exact_match() {
        assert!(glob_matches("src/main.rs", "src/main.rs"));
    }

    #[test]
    fn glob_star_matches_any_sequence() {
        assert!(glob_matches("*.rs", "main.rs"));
        assert!(glob_matches("src/*", "src/main.rs"));
        assert!(glob_matches("*main*", "src/main.rs"));
    }

    #[test]
    fn glob_question_mark_matches_single_char() {
        assert!(glob_matches("main.?s", "main.rs"));
        assert!(!glob_matches("main.?s", "main.rrs"));
    }

    #[test]
    fn glob_no_match() {
        assert!(!glob_matches("*.py", "main.rs"));
        assert!(!glob_matches("lib/*", "src/main.rs"));
    }

    #[test]
    fn glob_star_matches_across_slashes() {
        assert!(glob_matches("*.rs", "src/deep/main.rs"));
    }
}
