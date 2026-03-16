use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::error::{Result, RftError};

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub is_main: bool,
    pub index: usize,
}

pub async fn get_repo_root(cwd: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .await?;

    if !output.status.success() {
        return Err(RftError::NotAGitRepo);
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

pub fn get_repo_name(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn get_worktrees(repo_root: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(RftError::CommandFailed {
            cmd: "git worktree list --porcelain".to_string(),
            stderr,
        });
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    parse_porcelain_output(&raw)
}

pub async fn get_non_main_worktrees(repo_root: &Path) -> Result<Vec<WorktreeInfo>> {
    let worktrees = get_worktrees(repo_root).await?;
    Ok(worktrees.into_iter().filter(|wt| !wt.is_main).collect())
}

pub async fn get_worktree_by_index(repo_root: &Path, index: usize) -> Result<WorktreeInfo> {
    let worktrees = get_worktrees(repo_root).await?;
    worktrees
        .into_iter()
        .find(|wt| wt.index == index)
        .ok_or(RftError::WorktreeNotFound { index })
}

fn parse_porcelain_output(raw: &str) -> Result<Vec<WorktreeInfo>> {
    let mut worktrees = Vec::new();
    let blocks = raw.split("\n\n").filter(|block| !block.trim().is_empty());
    let mut non_main_counter = 0usize;

    for (position, block) in blocks.enumerate() {
        let mut path: Option<PathBuf> = None;
        let mut branch = String::from("detached");

        for line in block.lines() {
            if let Some(worktree_path) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(worktree_path));
            } else if let Some(branch_ref) = line.strip_prefix("branch ") {
                branch = branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string();
            }
        }

        let is_main = position == 0;

        if let Some(path) = path {
            let index = if is_main {
                0
            } else {
                non_main_counter += 1;
                non_main_counter
            };

            worktrees.push(WorktreeInfo {
                path,
                branch,
                is_main,
                index,
            });
        }
    }

    Ok(worktrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORCELAIN_TWO_WORKTREES: &str = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/main

worktree /home/user/project-feature
HEAD 789def012abc
branch refs/heads/feature/login
";

    const PORCELAIN_WITH_DETACHED: &str = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/main

worktree /home/user/project-detached
HEAD 789def012abc
detached
";

    const PORCELAIN_SINGLE: &str = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/main
";

    #[test]
    fn parse_two_worktrees() {
        let worktrees = parse_porcelain_output(PORCELAIN_TWO_WORKTREES).unwrap();

        assert_eq!(worktrees.len(), 2);

        assert_eq!(worktrees[0].path, PathBuf::from("/home/user/project"));
        assert_eq!(worktrees[0].branch, "main");
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].index, 0);

        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/home/user/project-feature")
        );
        assert_eq!(worktrees[1].branch, "feature/login");
        assert!(!worktrees[1].is_main);
        assert_eq!(worktrees[1].index, 1, "first non-main worktree should be 1-indexed");
    }

    #[test]
    fn parse_detached_head() {
        let worktrees = parse_porcelain_output(PORCELAIN_WITH_DETACHED).unwrap();

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[1].branch, "detached");
        assert!(!worktrees[1].is_main);
        assert_eq!(worktrees[1].index, 1, "detached non-main worktree is 1-indexed");
    }

    #[test]
    fn parse_single_worktree() {
        let worktrees = parse_porcelain_output(PORCELAIN_SINGLE).unwrap();

        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].branch, "main");
    }

    #[test]
    fn parse_empty_output() {
        let worktrees = parse_porcelain_output("").unwrap();
        assert!(worktrees.is_empty());
    }

    #[test]
    fn repo_name_from_path() {
        assert_eq!(get_repo_name(Path::new("/home/user/my-project")), "my-project");
        assert_eq!(get_repo_name(Path::new("/")), "unknown");
    }
}
