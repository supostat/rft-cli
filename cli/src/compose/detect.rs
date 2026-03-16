use std::path::{Path, PathBuf};

const COMPOSE_FILENAMES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

pub fn detect_compose_file(repo_root: &Path) -> Option<PathBuf> {
    COMPOSE_FILENAMES
        .iter()
        .map(|name| repo_root.join(name))
        .find(|path| path.is_file())
}
