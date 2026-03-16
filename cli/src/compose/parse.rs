use std::path::{Path, PathBuf};

use serde_yml::Value;

use crate::error::{Result, RftError};

#[derive(Debug, Clone)]
pub struct ComposeService {
    pub name: String,
    pub ports: Vec<String>,
    pub build: Option<BuildConfig>,
    pub env_file: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComposeFile {
    pub services: Vec<ComposeService>,
    pub compose_path: PathBuf,
}

pub fn parse_compose_file(path: &Path) -> Result<ComposeFile> {
    let content = std::fs::read_to_string(path)?;
    let root: Value = serde_yml::from_str(&content)?;

    let services_map = root
        .get("services")
        .and_then(Value::as_mapping)
        .ok_or_else(|| RftError::Config("missing or invalid 'services' key".into()))?;

    let mut services = Vec::with_capacity(services_map.len());

    for (key, value) in services_map {
        let name = key
            .as_str()
            .ok_or_else(|| RftError::Config("service name must be a string".into()))?
            .to_owned();

        let ports = extract_ports(value);
        let build = extract_build(value);
        let env_file = extract_env_file(value);

        services.push(ComposeService {
            name,
            ports,
            build,
            env_file,
        });
    }

    Ok(ComposeFile {
        services,
        compose_path: path.to_path_buf(),
    })
}

fn extract_ports(service: &Value) -> Vec<String> {
    let Some(ports_value) = service.get("ports") else {
        return Vec::new();
    };

    let Some(ports_seq) = ports_value.as_sequence() else {
        return Vec::new();
    };

    ports_seq.iter().filter_map(port_entry_to_string).collect()
}

fn port_entry_to_string(entry: &Value) -> Option<String> {
    match entry {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Mapping(map) => {
            let target = map.get(Value::String("target".into()))?.as_u64()?;
            let published = map
                .get(Value::String("published".into()))
                .and_then(|v| v.as_u64().or_else(|| v.as_str().map(|s| s.parse().ok())?));

            match published {
                Some(host) => Some(format!("{host}:{target}")),
                None => Some(target.to_string()),
            }
        }
        _ => None,
    }
}

fn extract_build(service: &Value) -> Option<BuildConfig> {
    let build_value = service.get("build")?;

    match build_value {
        Value::String(context) => Some(BuildConfig {
            context: Some(context.clone()),
            dockerfile: None,
        }),
        Value::Mapping(map) => {
            let context = map
                .get(Value::String("context".into()))
                .and_then(Value::as_str)
                .map(String::from);
            let dockerfile = map
                .get(Value::String("dockerfile".into()))
                .and_then(Value::as_str)
                .map(String::from);
            Some(BuildConfig {
                context,
                dockerfile,
            })
        }
        _ => None,
    }
}

fn extract_env_file(service: &Value) -> Vec<String> {
    let Some(env_value) = service.get("env_file") else {
        return Vec::new();
    };

    match env_value {
        Value::String(s) => vec![s.clone()],
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_compose(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
        let path = dir.path().join(filename);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parse_env_var_ports() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  frontend:
    image: node:20
    ports:
      - "${FRONTEND_PORT:-3000}:3000"
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        assert_eq!(compose.services.len(), 1);
        assert_eq!(compose.services[0].name, "frontend");
        assert_eq!(
            compose.services[0].ports,
            vec!["${FRONTEND_PORT:-3000}:3000"]
        );
    }

    #[test]
    fn parse_numeric_ports() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  api:
    image: python:3.12
    ports:
      - 8080
      - "9090:9090"
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        assert_eq!(compose.services[0].ports, vec!["8080", "9090:9090"]);
    }

    #[test]
    fn parse_long_form_ports() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  web:
    image: nginx
    ports:
      - target: 80
        published: 8080
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        assert_eq!(compose.services[0].ports, vec!["8080:80"]);
    }

    #[test]
    fn parse_build_as_string() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  app:
    build: ./app
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        let build = compose.services[0].build.as_ref().unwrap();
        assert_eq!(build.context.as_deref(), Some("./app"));
        assert!(build.dockerfile.is_none());
    }

    #[test]
    fn parse_build_as_object() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  app:
    build:
      context: ./app
      dockerfile: Dockerfile.prod
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        let build = compose.services[0].build.as_ref().unwrap();
        assert_eq!(build.context.as_deref(), Some("./app"));
        assert_eq!(build.dockerfile.as_deref(), Some("Dockerfile.prod"));
    }

    #[test]
    fn parse_env_file_as_string() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  app:
    image: node:20
    env_file: .env
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        assert_eq!(compose.services[0].env_file, vec![".env"]);
    }

    #[test]
    fn parse_env_file_as_array() {
        let dir = TempDir::new().unwrap();
        let path = write_compose(
            &dir,
            "compose.yaml",
            r#"
services:
  app:
    image: node:20
    env_file:
      - .env
      - .env.local
"#,
        );

        let compose = parse_compose_file(&path).unwrap();
        assert_eq!(compose.services[0].env_file, vec![".env", ".env.local"]);
    }

    #[test]
    fn detect_compose_file_priority() {
        use crate::compose::detect::detect_compose_file;

        let dir = TempDir::new().unwrap();

        assert!(detect_compose_file(dir.path()).is_none());

        std::fs::File::create(dir.path().join("docker-compose.yml")).unwrap();
        assert_eq!(
            detect_compose_file(dir.path()).unwrap(),
            dir.path().join("docker-compose.yml")
        );

        std::fs::File::create(dir.path().join("compose.yaml")).unwrap();
        assert_eq!(
            detect_compose_file(dir.path()).unwrap(),
            dir.path().join("compose.yaml")
        );
    }
}
