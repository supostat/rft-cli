use std::path::PathBuf;

use crate::compose::ComposeFile;
use crate::config::RftConfig;
use crate::error::Result;
use crate::git::WorktreeInfo;
use crate::ports::PortMapping;

#[derive(Debug)]
pub struct RftContext {
    pub repo_root: PathBuf,
    pub repo_name: String,
    pub compose_file: ComposeFile,
    pub port_mappings: Vec<PortMapping>,
    pub config: RftConfig,
    pub worktrees: Vec<WorktreeInfo>,
}

pub async fn build_context() -> Result<RftContext> {
    let cwd = std::env::current_dir()?;
    build_context_from(&cwd).await
}

pub async fn build_context_from(cwd: &std::path::Path) -> Result<RftContext> {
    let identity = crate::git::resolve_repo_identity(cwd).await?;
    let repo_root = identity.working_root;
    let repo_name = identity.project_name;

    let compose_path = crate::compose::detect_compose_file(&repo_root).ok_or_else(|| {
        crate::error::RftError::ComposeNotFound {
            path: repo_root.clone(),
        }
    })?;
    let compose_path_clone = compose_path.clone();
    let compose_file = tokio::task::spawn_blocking(move || {
        crate::compose::parse_compose_file(&compose_path_clone)
    })
    .await
    .map_err(|e| crate::error::RftError::TaskPanicked(e.to_string()))??;

    let port_mappings = crate::ports::extract_port_mappings(&compose_file.services);
    let repo_root_clone = repo_root.clone();
    let config = tokio::task::spawn_blocking(move || crate::config::load_config(&repo_root_clone))
        .await
        .map_err(|e| crate::error::RftError::TaskPanicked(e.to_string()))?;
    let worktrees = crate::git::get_worktrees(&repo_root, config.main_branch.as_deref()).await?;

    Ok(RftContext {
        repo_root,
        repo_name,
        compose_file,
        port_mappings,
        config,
        worktrees,
    })
}

pub fn filter_worktrees<'a>(
    worktrees: &'a [WorktreeInfo],
    indices: &[usize],
) -> Vec<&'a WorktreeInfo> {
    if indices.is_empty() {
        return worktrees.iter().filter(|wt| !wt.is_main).collect();
    }

    worktrees
        .iter()
        .filter(|wt| !wt.is_main && indices.contains(&wt.index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::WorktreeInfo;

    fn sample_worktrees() -> Vec<WorktreeInfo> {
        vec![
            WorktreeInfo {
                path: PathBuf::from("/repo"),
                branch: "main".to_string(),
                is_main: true,
                index: 0, // main always 0
            },
            WorktreeInfo {
                path: PathBuf::from("/repo-wt-1"),
                branch: "feature-a".to_string(),
                is_main: false,
                index: 1, // 1-indexed for non-main
            },
            WorktreeInfo {
                path: PathBuf::from("/repo-wt-2"),
                branch: "feature-b".to_string(),
                is_main: false,
                index: 2,
            },
        ]
    }

    #[test]
    fn filter_returns_all_non_main_when_indices_empty() {
        let worktrees = sample_worktrees();
        let filtered = filter_worktrees(&worktrees, &[]);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].branch, "feature-a");
        assert_eq!(filtered[1].branch, "feature-b");
    }

    #[test]
    fn filter_returns_specific_1_indexed() {
        let worktrees = sample_worktrees();
        let filtered = filter_worktrees(&worktrees, &[1]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].branch, "feature-a");
    }

    #[test]
    fn filter_returns_multiple_indices() {
        let worktrees = sample_worktrees();
        let filtered = filter_worktrees(&worktrees, &[1, 2]);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_ignores_invalid_indices() {
        let worktrees = sample_worktrees();
        let filtered = filter_worktrees(&worktrees, &[99]);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_skips_main_even_if_requested() {
        let worktrees = sample_worktrees();
        let filtered = filter_worktrees(&worktrees, &[0]);
        assert!(filtered.is_empty());
    }
}
