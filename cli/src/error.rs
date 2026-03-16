use std::path::PathBuf;

#[derive(thiserror::Error, miette::Diagnostic, Debug)]
pub enum RftError {
    #[error("no compose file found in {path}")]
    ComposeNotFound { path: PathBuf },

    #[error("no git repository found")]
    NotAGitRepo,

    #[error("worktree index {index} not found")]
    WorktreeNotFound { index: usize },

    #[error("port {port} out of valid range (1024-65535)")]
    PortOutOfRange { port: u32 },

    #[error("port collision: {port} assigned to multiple services")]
    PortCollision { port: u16 },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yml::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("command failed: {cmd}\n{stderr}")]
    CommandFailed { cmd: String, stderr: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("promote aborted: {reason}")]
    PromoteConflict { reason: String },

    #[error("task failed: {0}")]
    TaskPanicked(String),

    #[error("path traversal rejected: {path} escapes {root}")]
    PathTraversal { path: String, root: String },

    #[error("interrupted by user")]
    Interrupted,
}

/// Validate that `base.join(relative)` stays within `base`. Rejects `..` components and absolute paths.
pub fn validate_path_within(base: &std::path::Path, relative: &str) -> Result<std::path::PathBuf> {
    if std::path::Path::new(relative).is_absolute() {
        return Err(RftError::PathTraversal {
            path: relative.to_string(),
            root: base.display().to_string(),
        });
    }

    let joined = base.join(relative);
    let normalized = normalize_path(&joined);

    if !normalized.starts_with(base) {
        return Err(RftError::PathTraversal {
            path: relative.to_string(),
            root: base.display().to_string(),
        });
    }

    Ok(joined)
}

fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

pub type Result<T> = std::result::Result<T, RftError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn valid_relative_path() {
        let base = Path::new("/repo");
        let result = validate_path_within(base, "src/main.rs");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Path::new("/repo/src/main.rs"));
    }

    #[test]
    fn valid_nested_path() {
        let base = Path::new("/repo");
        assert!(validate_path_within(base, "deep/nested/file.txt").is_ok());
    }

    #[test]
    fn rejects_parent_traversal() {
        let base = Path::new("/repo");
        let result = validate_path_within(base, "../../etc/passwd");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RftError::PathTraversal { .. }
        ));
    }

    #[test]
    fn rejects_sneaky_traversal() {
        let base = Path::new("/repo");
        let result = validate_path_within(base, "subdir/../../outside");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        let base = Path::new("/repo");
        let result = validate_path_within(base, "/etc/shadow");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RftError::PathTraversal { .. }
        ));
    }

    #[test]
    fn allows_current_dir_component() {
        let base = Path::new("/repo");
        assert!(validate_path_within(base, "./src/main.rs").is_ok());
    }

    #[test]
    fn allows_simple_filename() {
        let base = Path::new("/repo");
        assert!(validate_path_within(base, "Makefile").is_ok());
    }
}
