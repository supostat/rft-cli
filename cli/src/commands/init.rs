use owo_colors::OwoColorize;

use crate::compose::{detect_compose_file, parse_compose_file};
use crate::error::Result;
use crate::git::{get_repo_name, get_repo_root};
use crate::ports::extract_port_mappings;

pub async fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = get_repo_root(&cwd).await?;
    let repo_name = get_repo_name(&repo_root);

    println!(
        "{} {} ({})",
        "Initializing rft for".green().bold(),
        repo_name.bold(),
        repo_root.display()
    );

    let compose_path = detect_compose_file(&repo_root);

    let compose_path = match compose_path {
        Some(path) => {
            println!(
                "  {} Found {}",
                "✓".green(),
                path.strip_prefix(&repo_root).unwrap_or(&path).display()
            );
            path
        }
        None => {
            eprintln!(
                "  {} No compose file found (compose.yaml, docker-compose.yml, etc.)",
                "✗".red()
            );
            eprintln!(
                "\n{}",
                "Create a docker-compose.yml first, then run rft init again.".dimmed()
            );
            return Ok(());
        }
    };

    let compose_file = tokio::task::spawn_blocking({
        let path = compose_path.clone();
        move || parse_compose_file(&path)
    })
    .await
    .map_err(|e| crate::error::RftError::TaskPanicked(e.to_string()))??;

    let port_mappings = extract_port_mappings(&compose_file.services);

    let managed_count = port_mappings.iter().filter(|m| m.env_var.is_some()).count();
    let raw_count = port_mappings.iter().filter(|m| m.env_var.is_none()).count();

    println!(
        "  {} {} service(s), {} port mapping(s)",
        "✓".green(),
        compose_file.services.len(),
        port_mappings.len()
    );

    if managed_count > 0 {
        println!(
            "  {} {} port(s) using ${{VAR:-default}} format (managed by rft)",
            "✓".green(),
            managed_count
        );
    }

    if raw_count > 0 {
        println!(
            "\n{}",
            "The following ports use hardcoded values. rft cannot override them:"
                .yellow()
                .bold()
        );
        for mapping in port_mappings.iter().filter(|m| m.env_var.is_none()) {
            let suggested =
                crate::ports::suggest_env_var(&mapping.service_name, mapping.default_port);
            println!(
                "  {} {} port {} → change to ${{{}:-{}}}:{}",
                "→".yellow(),
                mapping.service_name,
                mapping.raw,
                suggested,
                mapping.default_port,
                mapping.container_port
            );
        }
    }

    let config_path = repo_root.join(".rftrc.toml");
    if config_path.exists() {
        println!("\n  {} .rftrc.toml already exists", "✓".green());
    } else if managed_count > 0 {
        tokio::fs::write(&config_path, "# rft configuration\n# See: https://github.com/supostat/rft-cli\n\n# sync = []\n# port_offset = 20000\n# main_branch = \"main\"\n").await?;
        println!("\n  {} Created .rftrc.toml", "✓".green());
    }

    if managed_count == 0 && raw_count > 0 {
        println!(
            "\n{}",
            "No ports use ${VAR:-default} format. Fix the ports above, then run rft init again."
                .yellow()
        );
    } else {
        println!(
            "\n{} Run {} to see your worktrees.",
            "Ready!".green().bold(),
            "rft list".bold()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ports::suggest_env_var;

    #[test]
    fn suggest_env_var_for_common_services() {
        assert_eq!(suggest_env_var("frontend", 3000), "FRONTEND_PORT_3000");
        assert_eq!(suggest_env_var("api", 8080), "API_PORT_8080");
        assert_eq!(suggest_env_var("postgres", 5432), "POSTGRES_PORT_5432");
    }
}
