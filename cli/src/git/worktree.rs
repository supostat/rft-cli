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

impl WorktreeInfo {
    pub fn dir_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.branch.clone())
    }

    pub fn project_label(&self, source: &crate::config::ProjectNameSource) -> String {
        match source {
            crate::config::ProjectNameSource::Directory => self.dir_name(),
            crate::config::ProjectNameSource::Branch => self.branch.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepoIdentity {
    pub working_root: PathBuf,
    pub project_name: String,
}

pub async fn resolve_repo_identity(cwd: &Path) -> Result<RepoIdentity> {
    let common_dir = get_git_common_dir(cwd).await?;
    let project_name = project_name_from_common_dir(&common_dir);

    let working_root = match try_show_toplevel(cwd).await {
        Some(root) => root,
        None => find_main_worktree_path(cwd).await?,
    };

    Ok(RepoIdentity {
        working_root,
        project_name,
    })
}

async fn get_git_common_dir(cwd: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(cwd)
        .output()
        .await?;

    if !output.status.success() {
        return Err(RftError::NotAGitRepo);
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = PathBuf::from(&raw);

    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(cwd
            .join(&path)
            .canonicalize()
            .unwrap_or_else(|_| cwd.join(&path)))
    }
}

fn project_name_from_common_dir(common_dir: &Path) -> String {
    let Some(dir_name) = common_dir.file_name() else {
        return "unknown".to_string();
    };
    let dir_name = dir_name.to_string_lossy();

    if dir_name.starts_with('.') {
        common_dir
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        dir_name.into_owned()
    }
}

async fn try_show_toplevel(cwd: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(PathBuf::from(root))
}

async fn find_main_worktree_path(cwd: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(cwd)
        .output()
        .await?;

    if !output.status.success() {
        return Err(RftError::NotAGitRepo);
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let worktrees = parse_porcelain_output(&raw, None)?;

    worktrees
        .iter()
        .find(|wt| wt.is_main)
        .or_else(|| worktrees.first())
        .map(|wt| wt.path.clone())
        .ok_or(RftError::NoMainWorktree)
}

pub async fn get_worktrees(
    repo_root: &Path,
    main_branch: Option<&str>,
) -> Result<Vec<WorktreeInfo>> {
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
    parse_porcelain_output(&raw, main_branch)
}

pub async fn get_worktree_by_index(
    repo_root: &Path,
    index: usize,
    main_branch: Option<&str>,
) -> Result<WorktreeInfo> {
    let worktrees = get_worktrees(repo_root, main_branch).await?;
    worktrees
        .into_iter()
        .find(|wt| !wt.is_main && wt.index == index)
        .ok_or(RftError::WorktreeNotFound { index })
}

const MAIN_BRANCH_NAMES: &[&str] = &["main", "master"];

fn parse_porcelain_output(raw: &str, main_branch: Option<&str>) -> Result<Vec<WorktreeInfo>> {
    let mut worktrees = Vec::new();
    let blocks = raw.split("\n\n").filter(|block| !block.trim().is_empty());
    let mut non_main_counter = 0usize;

    for block in blocks {
        let mut path: Option<PathBuf> = None;
        let mut branch = String::from("detached");
        let mut is_bare = false;

        for line in block.lines() {
            if let Some(worktree_path) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(worktree_path));
            } else if let Some(branch_ref) = line.strip_prefix("branch ") {
                branch = branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string();
            } else if line.trim() == "bare" {
                is_bare = true;
            }
        }

        if is_bare {
            continue;
        }

        let is_main = match main_branch {
            Some(name) => branch == name,
            None => MAIN_BRANCH_NAMES.contains(&branch.as_str()),
        };

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

    const PORCELAIN_BARE_REPO: &str = "\
worktree /home/user/project/.bare
bare

worktree /home/user/project/main
HEAD abc123def456
branch refs/heads/main

worktree /home/user/project/feature-auth
HEAD 789def012abc
branch refs/heads/feature/auth
";

    const PORCELAIN_MASTER_BRANCH: &str = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/master

worktree /home/user/project-feature
HEAD 789def012abc
branch refs/heads/feature/login
";

    #[test]
    fn parse_two_worktrees() {
        let worktrees = parse_porcelain_output(PORCELAIN_TWO_WORKTREES, None).unwrap();

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
        assert_eq!(
            worktrees[1].index, 1,
            "first non-main worktree should be 1-indexed"
        );
    }

    #[test]
    fn parse_detached_head() {
        let worktrees = parse_porcelain_output(PORCELAIN_WITH_DETACHED, None).unwrap();

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[1].branch, "detached");
        assert!(!worktrees[1].is_main);
        assert_eq!(
            worktrees[1].index, 1,
            "detached non-main worktree is 1-indexed"
        );
    }

    #[test]
    fn parse_single_worktree() {
        let worktrees = parse_porcelain_output(PORCELAIN_SINGLE, None).unwrap();

        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].branch, "main");
    }

    #[test]
    fn parse_empty_output() {
        let worktrees = parse_porcelain_output("", None).unwrap();
        assert!(worktrees.is_empty());
    }

    #[test]
    fn parse_bare_repo_skips_bare_entry() {
        let worktrees = parse_porcelain_output(PORCELAIN_BARE_REPO, None).unwrap();

        assert_eq!(worktrees.len(), 2, "bare entry should be skipped");
        assert_eq!(worktrees[0].branch, "main");
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].index, 0);

        assert_eq!(worktrees[1].branch, "feature/auth");
        assert!(!worktrees[1].is_main);
        assert_eq!(worktrees[1].index, 1);
    }

    #[test]
    fn parse_master_branch_is_main() {
        let worktrees = parse_porcelain_output(PORCELAIN_MASTER_BRANCH, None).unwrap();

        assert_eq!(worktrees.len(), 2);
        assert!(worktrees[0].is_main, "master should be detected as main");
        assert_eq!(worktrees[0].index, 0);
        assert!(!worktrees[1].is_main);
        assert_eq!(worktrees[1].index, 1);
    }

    #[test]
    fn custom_main_branch_from_config() {
        let porcelain = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/develop

worktree /home/user/project-feature
HEAD 789def012abc
branch refs/heads/feature/login
";
        let worktrees = parse_porcelain_output(porcelain, Some("develop")).unwrap();

        assert_eq!(worktrees.len(), 2);
        assert!(
            worktrees[0].is_main,
            "develop should be main when configured"
        );
        assert_eq!(worktrees[0].index, 0);
        assert!(!worktrees[1].is_main);
    }

    #[test]
    fn custom_main_branch_does_not_match_default() {
        let worktrees = parse_porcelain_output(PORCELAIN_TWO_WORKTREES, Some("develop")).unwrap();

        assert!(
            !worktrees[0].is_main,
            "main branch should not match when config says develop"
        );
    }

    #[test]
    fn project_name_from_dot_git() {
        let name = project_name_from_common_dir(Path::new("/home/user/myapp/.git"));
        assert_eq!(name, "myapp");
    }

    #[test]
    fn project_name_from_dot_bare() {
        let name = project_name_from_common_dir(Path::new("/home/user/myapp/.bare"));
        assert_eq!(name, "myapp");
    }

    #[test]
    fn project_name_from_plain_bare() {
        let name = project_name_from_common_dir(Path::new("/home/user/boss"));
        assert_eq!(name, "boss");
    }

    #[test]
    fn project_name_from_root_path() {
        let name = project_name_from_common_dir(Path::new("/"));
        assert_eq!(name, "unknown");
    }

    #[test]
    fn project_name_from_dot_hidden_custom() {
        let name = project_name_from_common_dir(Path::new("/projects/myapp/.gitdata"));
        assert_eq!(name, "myapp");
    }
}
