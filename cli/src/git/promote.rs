use std::collections::HashSet;
use std::path::Path;

use tokio::process::Command;

use crate::error::{Result, RftError};

const EXCLUDED_FILES: &[&str] = &[
    ".env",
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub source: ChangeSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeSource {
    Committed,
    Uncommitted,
    Untracked,
}

#[derive(Debug, Default)]
pub struct PromoteResult {
    pub git_checkout_count: usize,
    pub file_copy_count: usize,
    pub files: Vec<PromotedFile>,
}

#[derive(Debug)]
pub struct PromotedFile {
    pub path: String,
    pub method: PromoteMethod,
}

#[derive(Debug, PartialEq)]
pub enum PromoteMethod {
    GitCheckout,
    FileCopy,
}

fn is_excluded(path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path);
    EXCLUDED_FILES.contains(&path) || EXCLUDED_FILES.contains(&basename)
}

fn parse_file_list(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

async fn run_git_command(working_dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(RftError::CommandFailed {
            cmd: format!("git {}", args.join(" ")),
            stderr,
        });
    }

    Ok(output.stdout)
}

pub async fn get_changed_files(
    worktree_path: &Path,
    worktree_branch: &str,
    main_branch: &str,
) -> Result<Vec<ChangedFile>> {
    let merge_base_output =
        run_git_command(worktree_path, &["merge-base", main_branch, worktree_branch]).await?;
    let merge_base = String::from_utf8_lossy(&merge_base_output)
        .trim()
        .to_string();

    let committed_output =
        run_git_command(worktree_path, &["diff", "--name-only", &merge_base, "HEAD"]).await?;
    let committed_files = parse_file_list(&committed_output);

    let uncommitted_output =
        run_git_command(worktree_path, &["diff", "--name-only", "HEAD"]).await?;
    let uncommitted_files = parse_file_list(&uncommitted_output);

    let untracked_output = run_git_command(
        worktree_path,
        &["ls-files", "--others", "--exclude-standard"],
    )
    .await?;
    let untracked_files = parse_file_list(&untracked_output);

    deduplicate_changed_files(&committed_files, &uncommitted_files, &untracked_files)
}

fn deduplicate_changed_files(
    committed: &[String],
    uncommitted: &[String],
    untracked: &[String],
) -> Result<Vec<ChangedFile>> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for path in committed {
        if !is_excluded(path) && seen.insert(path.clone()) {
            result.push(ChangedFile {
                path: path.clone(),
                source: ChangeSource::Committed,
            });
        }
    }

    for path in uncommitted {
        if !is_excluded(path) && seen.insert(path.clone()) {
            result.push(ChangedFile {
                path: path.clone(),
                source: ChangeSource::Uncommitted,
            });
        }
    }

    for path in untracked {
        if !is_excluded(path) && seen.insert(path.clone()) {
            result.push(ChangedFile {
                path: path.clone(),
                source: ChangeSource::Untracked,
            });
        }
    }

    Ok(result)
}

pub async fn get_local_dirty_files(repo_root: &Path) -> Result<Vec<String>> {
    let unstaged_output = run_git_command(repo_root, &["diff", "--name-only", "HEAD"]).await?;
    let staged_output = run_git_command(repo_root, &["diff", "--name-only", "--cached"]).await?;

    let mut files = HashSet::new();
    for path in parse_file_list(&unstaged_output) {
        files.insert(path);
    }
    for path in parse_file_list(&staged_output) {
        files.insert(path);
    }

    Ok(files.into_iter().collect())
}

pub fn find_conflicts(changed: &[ChangedFile], dirty: &[String]) -> Vec<String> {
    let dirty_set: HashSet<&str> = dirty.iter().map(|s| s.as_str()).collect();
    changed
        .iter()
        .filter(|file| dirty_set.contains(file.path.as_str()))
        .map(|file| file.path.clone())
        .collect()
}

pub async fn promote_files(
    repo_root: &Path,
    worktree_path: &Path,
    worktree_branch: &str,
    files: &[ChangedFile],
) -> Result<PromoteResult> {
    let mut result = PromoteResult::default();

    let committed_files: Vec<&ChangedFile> = files
        .iter()
        .filter(|f| f.source == ChangeSource::Committed)
        .collect();

    if !committed_files.is_empty() {
        let mut args = vec!["checkout", worktree_branch, "--"];
        let paths: Vec<&str> = committed_files.iter().map(|f| f.path.as_str()).collect();
        args.extend(paths);

        run_git_command(repo_root, &args).await?;

        for file in &committed_files {
            result.git_checkout_count += 1;
            result.files.push(PromotedFile {
                path: file.path.clone(),
                method: PromoteMethod::GitCheckout,
            });
        }
    }

    let copy_files: Vec<&ChangedFile> = files
        .iter()
        .filter(|f| f.source != ChangeSource::Committed)
        .collect();

    for file in &copy_files {
        crate::error::validate_path_within(worktree_path, &file.path)?;
        crate::error::validate_path_within(repo_root, &file.path)?;

        let source = worktree_path.join(&file.path);
        let destination = repo_root.join(&file.path);

        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::copy(&source, &destination).await?;

        result.file_copy_count += 1;
        result.files.push(PromotedFile {
            path: file.path.clone(),
            method: PromoteMethod::FileCopy,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_conflicts_detects_overlapping_files() {
        let changed = vec![
            ChangedFile {
                path: "src/main.rs".to_string(),
                source: ChangeSource::Committed,
            },
            ChangedFile {
                path: "src/lib.rs".to_string(),
                source: ChangeSource::Uncommitted,
            },
            ChangedFile {
                path: "README.md".to_string(),
                source: ChangeSource::Untracked,
            },
        ];
        let dirty = vec!["src/lib.rs".to_string(), "Cargo.toml".to_string()];

        let conflicts = find_conflicts(&changed, &dirty);

        assert_eq!(conflicts, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn find_conflicts_returns_empty_when_no_overlap() {
        let changed = vec![ChangedFile {
            path: "src/main.rs".to_string(),
            source: ChangeSource::Committed,
        }];
        let dirty = vec!["Cargo.toml".to_string()];

        let conflicts = find_conflicts(&changed, &dirty);

        assert!(conflicts.is_empty());
    }

    #[test]
    fn find_conflicts_handles_empty_inputs() {
        assert!(find_conflicts(&[], &[]).is_empty());
        assert!(find_conflicts(&[], &["a.rs".to_string()]).is_empty());

        let changed = vec![ChangedFile {
            path: "a.rs".to_string(),
            source: ChangeSource::Committed,
        }];
        assert!(find_conflicts(&changed, &[]).is_empty());
    }

    #[test]
    fn excluded_files_are_filtered_out() {
        let committed = vec![
            "src/main.rs".to_string(),
            ".env".to_string(),
            "compose.yaml".to_string(),
            "compose.yml".to_string(),
            "docker-compose.yaml".to_string(),
            "docker-compose.yml".to_string(),
        ];

        let result = deduplicate_changed_files(&committed, &[], &[]).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "src/main.rs");
    }

    #[test]
    fn deduplication_prefers_committed_over_uncommitted_and_untracked() {
        let committed = vec!["shared.rs".to_string()];
        let uncommitted = vec!["shared.rs".to_string(), "only_uncommitted.rs".to_string()];
        let untracked = vec!["shared.rs".to_string(), "only_untracked.rs".to_string()];

        let result = deduplicate_changed_files(&committed, &uncommitted, &untracked).unwrap();

        assert_eq!(result.len(), 3);

        let shared = result.iter().find(|f| f.path == "shared.rs").unwrap();
        assert_eq!(shared.source, ChangeSource::Committed);

        let uncommitted_only = result
            .iter()
            .find(|f| f.path == "only_uncommitted.rs")
            .unwrap();
        assert_eq!(uncommitted_only.source, ChangeSource::Uncommitted);

        let untracked_only = result
            .iter()
            .find(|f| f.path == "only_untracked.rs")
            .unwrap();
        assert_eq!(untracked_only.source, ChangeSource::Untracked);
    }

    #[test]
    fn deduplication_prefers_uncommitted_over_untracked() {
        let committed = vec![];
        let uncommitted = vec!["file.rs".to_string()];
        let untracked = vec!["file.rs".to_string()];

        let result = deduplicate_changed_files(&committed, &uncommitted, &untracked).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, ChangeSource::Uncommitted);
    }

    #[test]
    fn parse_file_list_handles_various_formats() {
        assert!(parse_file_list(b"").is_empty());
        assert_eq!(parse_file_list(b"file.rs\n"), vec!["file.rs"]);
        assert_eq!(parse_file_list(b"a.rs\nb.rs\n"), vec!["a.rs", "b.rs"]);
        assert_eq!(parse_file_list(b"  spaced.rs  \n"), vec!["spaced.rs"]);
    }

    #[test]
    fn is_excluded_matches_exact_filenames() {
        assert!(is_excluded(".env"));
        assert!(is_excluded("compose.yaml"));
        assert!(is_excluded("docker-compose.yml"));
        assert!(!is_excluded("src/main.rs"));
        assert!(!is_excluded(".env.example"));
    }

    #[test]
    fn is_excluded_matches_nested_paths_by_basename() {
        assert!(is_excluded("sub/compose.yaml"));
        assert!(is_excluded("deep/nested/.env"));
        assert!(is_excluded("dir/docker-compose.yml"));
        assert!(!is_excluded("sub/.env.local"));
    }
}
