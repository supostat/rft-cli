use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

const DEFAULT_HOST: &str = "localhost";

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ProjectNameSource {
    Branch,
    #[default]
    Directory,
}

#[derive(Debug, Clone)]
pub struct RftConfig {
    pub sync: Vec<String>,
    pub env_overrides: HashMap<String, String>,
    pub port_offset: Option<u32>,
    pub main_branch: Option<String>,
    pub host: String,
    pub project_name_source: ProjectNameSource,
}

impl Default for RftConfig {
    fn default() -> Self {
        Self {
            sync: Vec::new(),
            env_overrides: HashMap::new(),
            port_offset: None,
            main_branch: None,
            host: DEFAULT_HOST.to_string(),
            project_name_source: ProjectNameSource::default(),
        }
    }
}

#[derive(Deserialize)]
struct ConfigFile {
    sync: Option<Vec<String>>,
    env_overrides: Option<HashMap<String, String>>,
    port_offset: Option<u32>,
    main_branch: Option<String>,
    host: Option<String>,
    project_name_source: Option<String>,
}

#[derive(Deserialize)]
struct PackageJson {
    rft: Option<ConfigFile>,
}

fn parse_project_name_source(value: &str) -> ProjectNameSource {
    match value {
        "branch" => ProjectNameSource::Branch,
        _ => ProjectNameSource::Directory,
    }
}

impl From<ConfigFile> for RftConfig {
    fn from(file: ConfigFile) -> Self {
        Self {
            sync: file.sync.unwrap_or_default(),
            env_overrides: file.env_overrides.unwrap_or_default(),
            port_offset: file.port_offset,
            main_branch: file.main_branch,
            host: file.host.unwrap_or_else(|| DEFAULT_HOST.to_string()),
            project_name_source: file
                .project_name_source
                .as_deref()
                .map(parse_project_name_source)
                .unwrap_or_default(),
        }
    }
}

pub fn load_config(repo_root: &Path) -> RftConfig {
    let mut config = load_from_files(repo_root);
    apply_local_overrides(&mut config, repo_root);
    apply_env_overrides(&mut config, |key| std::env::var(key));
    config
}

fn load_from_files(repo_root: &Path) -> RftConfig {
    if let Some(config) = load_toml(repo_root) {
        return config;
    }
    if let Some(config) = load_json(repo_root) {
        return config;
    }
    if let Some(config) = load_package_json(repo_root) {
        return config;
    }
    RftConfig::default()
}

fn apply_local_overrides(config: &mut RftConfig, repo_root: &Path) {
    let path = repo_root.join(".rftrc.local.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return,
    };

    let local: ConfigFile = match toml::from_str(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("warning: failed to parse {}: {error}", path.display());
            return;
        }
    };

    if let Some(sync) = local.sync {
        config.sync = sync;
    }
    if let Some(env_overrides) = local.env_overrides {
        config.env_overrides.extend(env_overrides);
    }
    if let Some(port_offset) = local.port_offset {
        config.port_offset = Some(port_offset);
    }
    if let Some(main_branch) = local.main_branch {
        config.main_branch = Some(main_branch);
    }
    if let Some(host) = local.host {
        config.host = host;
    }
    if let Some(source) = local.project_name_source.as_deref() {
        config.project_name_source = parse_project_name_source(source);
    }
}

fn load_toml(repo_root: &Path) -> Option<RftConfig> {
    let path = repo_root.join(".rftrc.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    match toml::from_str::<ConfigFile>(&content) {
        Ok(parsed) => Some(parsed.into()),
        Err(error) => {
            eprintln!("warning: failed to parse {}: {error}", path.display());
            None
        }
    }
}

fn load_json(repo_root: &Path) -> Option<RftConfig> {
    let path = repo_root.join(".rftrc.json");
    let content = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<ConfigFile>(&content) {
        Ok(parsed) => Some(parsed.into()),
        Err(error) => {
            eprintln!("warning: failed to parse {}: {error}", path.display());
            None
        }
    }
}

fn load_package_json(repo_root: &Path) -> Option<RftConfig> {
    let path = repo_root.join("package.json");
    let content = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<PackageJson>(&content) {
        Ok(parsed) => parsed.rft.map(Into::into),
        Err(error) => {
            eprintln!("warning: failed to parse {}: {error}", path.display());
            None
        }
    }
}

fn apply_env_overrides(
    config: &mut RftConfig,
    env_var: impl Fn(&str) -> std::result::Result<String, std::env::VarError>,
) {
    if let Ok(value) = env_var("RFT_PORT_OFFSET") {
        match value.parse::<u32>() {
            Ok(offset) => config.port_offset = Some(offset),
            Err(error) => {
                eprintln!("warning: invalid RFT_PORT_OFFSET value '{value}': {error}");
            }
        }
    }

    if let Ok(value) = env_var("RFT_HOST") {
        config.host = value;
    }

    if let Ok(value) = env_var("RFT_SYNC") {
        config.sync = value
            .split(',')
            .map(|segment| segment.trim().to_string())
            .filter(|segment| !segment.is_empty())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_toml_config() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".rftrc.toml"),
            r#"
sync = ["docker-compose.yml", ".env"]
port_offset = 100

[env_overrides]
DATABASE_URL = "postgres://localhost/test"
"#,
        )
        .unwrap();

        let config = load_from_files(dir.path());

        assert_eq!(config.sync, vec!["docker-compose.yml", ".env"]);
        assert_eq!(config.port_offset, Some(100));
        assert_eq!(
            config.env_overrides.get("DATABASE_URL").unwrap(),
            "postgres://localhost/test"
        );
    }

    #[test]
    fn loads_json_config() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".rftrc.json"),
            r#"{
                "sync": ["Makefile"],
                "port_offset": 200
            }"#,
        )
        .unwrap();

        let config = load_from_files(dir.path());

        assert_eq!(config.sync, vec!["Makefile"]);
        assert_eq!(config.port_offset, Some(200));
    }

    #[test]
    fn loads_from_package_json() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
                "name": "my-app",
                "rft": {
                    "sync": ["tsconfig.json"],
                    "port_offset": 50
                }
            }"#,
        )
        .unwrap();

        let config = load_from_files(dir.path());

        assert_eq!(config.sync, vec!["tsconfig.json"]);
        assert_eq!(config.port_offset, Some(50));
    }

    #[test]
    fn toml_takes_priority_over_json() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".rftrc.toml"), r#"port_offset = 10"#).unwrap();
        fs::write(dir.path().join(".rftrc.json"), r#"{"port_offset": 20}"#).unwrap();

        let config = load_from_files(dir.path());

        assert_eq!(config.port_offset, Some(10));
    }

    #[test]
    fn returns_default_when_no_config_found() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_from_files(dir.path());

        assert!(config.sync.is_empty());
        assert!(config.env_overrides.is_empty());
        assert_eq!(config.port_offset, None);
    }

    #[test]
    fn returns_default_on_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".rftrc.toml"), "{{invalid toml").unwrap();

        let config = load_from_files(dir.path());

        assert!(config.sync.is_empty());
        assert_eq!(config.port_offset, None);
    }

    fn fake_env(
        overrides: &[(&str, &str)],
    ) -> impl Fn(&str) -> std::result::Result<String, std::env::VarError> {
        let map: HashMap<String, String> = overrides
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key: &str| map.get(key).cloned().ok_or(std::env::VarError::NotPresent)
    }

    #[test]
    fn local_toml_overrides_shared() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".rftrc.toml"),
            r#"
port_offset = 20000
sync = ["nginx/"]
main_branch = "main"
"#,
        )
        .unwrap();
        fs::write(
            dir.path().join(".rftrc.local.toml"),
            r#"
host = "192.168.1.50"
port_offset = 30000
"#,
        )
        .unwrap();

        let config = load_config(dir.path());

        assert_eq!(config.host, "192.168.1.50");
        assert_eq!(config.port_offset, Some(30000));
        assert_eq!(config.sync, vec!["nginx/"]);
        assert_eq!(config.main_branch, Some("main".to_string()));
    }

    #[test]
    fn local_toml_without_shared_works() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".rftrc.local.toml"), r#"host = "10.0.0.1""#).unwrap();

        let config = load_config(dir.path());

        assert_eq!(config.host, "10.0.0.1");
        assert_eq!(config.port_offset, None);
    }

    #[test]
    fn env_var_overrides_port_offset() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".rftrc.toml"), r#"port_offset = 10"#).unwrap();

        let mut config = load_from_files(dir.path());
        apply_env_overrides(&mut config, fake_env(&[("RFT_PORT_OFFSET", "999")]));

        assert_eq!(config.port_offset, Some(999));
    }

    #[test]
    fn env_var_overrides_sync() {
        let mut config = RftConfig::default();
        apply_env_overrides(&mut config, fake_env(&[("RFT_SYNC", "a.yml, b.yml")]));

        assert_eq!(config.sync, vec!["a.yml", "b.yml"]);
    }

    #[test]
    fn project_name_source_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".rftrc.toml"),
            r#"project_name_source = "directory""#,
        )
        .unwrap();

        let config = load_config(dir.path());

        assert_eq!(config.project_name_source, ProjectNameSource::Directory);
    }

    #[test]
    fn project_name_source_branch() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".rftrc.toml"),
            r#"project_name_source = "branch""#,
        )
        .unwrap();

        let config = load_config(dir.path());

        assert_eq!(config.project_name_source, ProjectNameSource::Branch);
    }

    #[test]
    fn project_name_source_defaults_to_directory() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_config(dir.path());

        assert_eq!(config.project_name_source, ProjectNameSource::Directory);
    }

    #[test]
    fn package_json_without_rft_field_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-app", "version": "1.0.0"}"#,
        )
        .unwrap();

        let config = load_from_files(dir.path());

        assert!(config.sync.is_empty());
        assert_eq!(config.port_offset, None);
    }
}
