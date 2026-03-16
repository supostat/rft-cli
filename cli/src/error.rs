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
}

pub type Result<T> = std::result::Result<T, RftError>;
