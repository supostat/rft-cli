use std::path::{Path, PathBuf};

use crate::compose::ComposeFile;
use crate::error::Result;

pub fn extract_dockerfile_paths(compose_file: &ComposeFile) -> Vec<PathBuf> {
    let compose_dir = compose_file.compose_path.parent().unwrap_or(Path::new("."));

    compose_file
        .services
        .iter()
        .filter_map(|service| {
            let build = service.build.as_ref()?;
            let context = build.context.as_deref().unwrap_or(".");
            let dockerfile = build.dockerfile.as_deref().unwrap_or("Dockerfile");
            Some(compose_dir.join(context).join(dockerfile))
        })
        .collect()
}

pub async fn sync_worktree_files(
    repo_root: &Path,
    worktree_path: &Path,
    compose_file: &ComposeFile,
    extra_sync: &[String],
) -> Result<()> {
    let relative_compose = compose_file
        .compose_path
        .strip_prefix(repo_root)
        .unwrap_or(&compose_file.compose_path);
    let target_compose = worktree_path.join(relative_compose);
    copy_file_with_parents(&compose_file.compose_path, &target_compose).await?;

    let dockerfile_paths = extract_dockerfile_paths(compose_file);
    for absolute_dockerfile in &dockerfile_paths {
        if !absolute_dockerfile.exists() {
            continue;
        }
        let relative = absolute_dockerfile
            .strip_prefix(repo_root)
            .unwrap_or(absolute_dockerfile);
        let target = worktree_path.join(relative);
        copy_file_with_parents(absolute_dockerfile, &target).await?;
    }

    for extra in extra_sync {
        let source = repo_root.join(extra);
        if !source.exists() {
            continue;
        }
        let target = worktree_path.join(extra);
        if source.is_dir() {
            copy_directory_recursive(&source, &target).await?;
        } else {
            copy_file_with_parents(&source, &target).await?;
        }
    }

    Ok(())
}

async fn copy_file_with_parents(source: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(source, target).await?;
    Ok(())
}

async fn copy_directory_recursive(source: &Path, target: &Path) -> Result<()> {
    tokio::fs::create_dir_all(target).await?;

    let mut entries = tokio::fs::read_dir(source).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        let target_path = target.join(entry.file_name());

        if entry_path.is_dir() {
            Box::pin(copy_directory_recursive(&entry_path, &target_path)).await?;
        } else {
            tokio::fs::copy(&entry_path, &target_path).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::{BuildConfig, ComposeService};

    fn make_compose_file(compose_path: PathBuf, services: Vec<ComposeService>) -> ComposeFile {
        ComposeFile {
            services,
            compose_path,
        }
    }

    #[test]
    fn extract_dockerfiles_from_build_context() {
        let compose = make_compose_file(
            PathBuf::from("/repo/compose.yaml"),
            vec![
                ComposeService {
                    name: "web".to_string(),
                    ports: vec![],
                    build: Some(BuildConfig {
                        context: Some("./web".to_string()),
                        dockerfile: Some("Dockerfile.prod".to_string()),
                    }),
                    env_file: vec![],
                },
                ComposeService {
                    name: "api".to_string(),
                    ports: vec![],
                    build: Some(BuildConfig {
                        context: Some("./api".to_string()),
                        dockerfile: None,
                    }),
                    env_file: vec![],
                },
            ],
        );

        let paths = extract_dockerfile_paths(&compose);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/repo/./web/Dockerfile.prod"));
        assert_eq!(paths[1], PathBuf::from("/repo/./api/Dockerfile"));
    }

    #[test]
    fn extract_dockerfiles_skips_services_without_build() {
        let compose = make_compose_file(
            PathBuf::from("/repo/compose.yaml"),
            vec![ComposeService {
                name: "redis".to_string(),
                ports: vec![],
                build: None,
                env_file: vec![],
            }],
        );

        let paths = extract_dockerfile_paths(&compose);
        assert!(paths.is_empty());
    }

    #[test]
    fn extract_dockerfiles_defaults_context_to_dot() {
        let compose = make_compose_file(
            PathBuf::from("/repo/compose.yaml"),
            vec![ComposeService {
                name: "app".to_string(),
                ports: vec![],
                build: Some(BuildConfig {
                    context: None,
                    dockerfile: None,
                }),
                env_file: vec![],
            }],
        );

        let paths = extract_dockerfile_paths(&compose);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/repo/./Dockerfile"));
    }

    #[tokio::test]
    async fn sync_copies_compose_file_to_worktree() {
        let repo = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();

        let compose_path = repo.path().join("compose.yaml");
        tokio::fs::write(&compose_path, "services: {}")
            .await
            .unwrap();

        let compose_file = make_compose_file(compose_path, vec![]);

        sync_worktree_files(repo.path(), worktree.path(), &compose_file, &[])
            .await
            .unwrap();

        let target = worktree.path().join("compose.yaml");
        assert!(target.exists());
        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "services: {}"
        );
    }

    #[tokio::test]
    async fn sync_copies_extra_files_and_directories() {
        let repo = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();

        let compose_path = repo.path().join("compose.yaml");
        tokio::fs::write(&compose_path, "services: {}")
            .await
            .unwrap();

        tokio::fs::write(repo.path().join("Makefile"), "all: build")
            .await
            .unwrap();

        let subdir = repo.path().join("config");
        tokio::fs::create_dir_all(&subdir).await.unwrap();
        tokio::fs::write(subdir.join("settings.toml"), "[app]")
            .await
            .unwrap();

        let compose_file = make_compose_file(compose_path, vec![]);

        let extra = vec!["Makefile".to_string(), "config".to_string()];
        sync_worktree_files(repo.path(), worktree.path(), &compose_file, &extra)
            .await
            .unwrap();

        assert!(worktree.path().join("Makefile").exists());
        assert!(worktree.path().join("config/settings.toml").exists());
    }

    #[tokio::test]
    async fn sync_skips_missing_extra_files() {
        let repo = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();

        let compose_path = repo.path().join("compose.yaml");
        tokio::fs::write(&compose_path, "services: {}")
            .await
            .unwrap();

        let compose_file = make_compose_file(compose_path, vec![]);

        let extra = vec!["nonexistent.txt".to_string()];
        let result = sync_worktree_files(repo.path(), worktree.path(), &compose_file, &extra).await;
        assert!(result.is_ok());
    }
}
