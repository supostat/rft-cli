use std::path::Path;
use std::process::Output;

use owo_colors::OwoColorize;

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Executor {
    Real,
    DryRun,
}

impl Executor {
    pub fn is_dry_run(&self) -> bool {
        *self == Self::DryRun
    }

    pub async fn copy_file(&self, source: &Path, target: &Path) -> Result<()> {
        match self {
            Self::Real => {
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(source, target).await?;
                Ok(())
            }
            Self::DryRun => {
                println!(
                    "  {} {} → {}",
                    "copy".dimmed(),
                    source.display(),
                    target.display()
                );
                Ok(())
            }
        }
    }

    pub async fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        match self {
            Self::Real => {
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(path, content).await?;
                Ok(())
            }
            Self::DryRun => {
                println!(
                    "  {} {} ({} bytes)",
                    "write".dimmed(),
                    path.display(),
                    content.len()
                );
                Ok(())
            }
        }
    }

    pub async fn create_dir_all(&self, path: &Path) -> Result<()> {
        match self {
            Self::Real => {
                tokio::fs::create_dir_all(path).await?;
                Ok(())
            }
            Self::DryRun => {
                println!("  {} {}", "mkdir".dimmed(), path.display());
                Ok(())
            }
        }
    }

    pub async fn run_docker(&self, args: &[&str], cwd: &Path) -> Result<Option<Output>> {
        match self {
            Self::Real => {
                let output = tokio::process::Command::new("docker")
                    .args(args)
                    .current_dir(cwd)
                    .output()
                    .await?;
                Ok(Some(output))
            }
            Self::DryRun => {
                println!("  {} docker {}", "exec".dimmed(), args.join(" "));
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_is_dry_run() {
        assert!(!Executor::Real.is_dry_run());
        assert!(Executor::DryRun.is_dry_run());
    }

    #[tokio::test]
    async fn dry_run_copy_does_not_create_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let target = dir.path().join("target.txt");
        tokio::fs::write(&source, "data").await.unwrap();

        Executor::DryRun.copy_file(&source, &target).await.unwrap();

        assert!(!target.exists());
    }

    #[tokio::test]
    async fn real_copy_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let target = dir.path().join("sub/target.txt");
        tokio::fs::write(&source, "data").await.unwrap();

        Executor::Real.copy_file(&source, &target).await.unwrap();

        assert_eq!(tokio::fs::read_to_string(&target).await.unwrap(), "data");
    }

    #[tokio::test]
    async fn dry_run_write_does_not_create_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.txt");

        Executor::DryRun.write_file(&path, "content").await.unwrap();

        assert!(!path.exists());
    }

    #[tokio::test]
    async fn dry_run_docker_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = Executor::DryRun
            .run_docker(&["ps"], dir.path())
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
