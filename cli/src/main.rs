use clap::{CommandFactory, Parser, Subcommand};
use miette::Result;

mod commands;
mod compose;
mod config;
mod context;
mod error;
mod git;
mod mcp;
mod ports;
mod sanitize;
mod sync;

#[derive(Parser)]
#[command(name = "rft", version, about = "Zero-config Docker Compose isolation for git worktrees")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start Docker Compose stacks for worktrees
    Start {
        /// Worktree indices to start (1-indexed, all if omitted)
        indices: Vec<usize>,
        /// Show what would be done without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Stop Docker Compose stacks
    Stop {
        /// Worktree indices to stop (1-indexed, all if omitted)
        indices: Vec<usize>,
    },
    /// Restart Docker Compose stacks
    Restart {
        /// Worktree indices to restart (1-indexed, all if omitted)
        indices: Vec<usize>,
    },
    /// List all worktrees with ports and status
    List,
    /// Promote changes from a worktree to current branch
    Promote {
        /// Worktree index (1-indexed)
        index: usize,
        /// Show what would be promoted without executing
        #[arg(long)]
        dry_run: bool,
        /// Only promote files matching glob pattern
        #[arg(long)]
        files: Option<String>,
    },
    /// Stop all stacks, remove worktrees, clean up Docker resources
    Clean,
    /// Show logs for a worktree stack
    Logs {
        /// Worktree index (1-indexed)
        index: usize,
        /// Service name (all services if omitted)
        service: Option<String>,
        /// Don't follow log output
        #[arg(long)]
        no_follow: bool,
    },
    /// One-line status for shell prompt integration
    Status,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
    /// Start MCP server (stdio transport)
    Mcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    miette::set_panic_hook();
    let cli = Cli::parse();

    match cli.command {
        Command::List => commands::list::run().await,
        Command::Start { indices, dry_run: _ } => {
            commands::start::run(indices).await.map_err(Into::into)
        }
        Command::Stop { indices } => commands::stop::run(indices).await.map_err(Into::into),
        Command::Restart { indices } => commands::restart::run(indices).await.map_err(Into::into),
        Command::Promote { index, dry_run, files } => {
            commands::promote::run(index, dry_run, files.as_deref()).await
        }
        Command::Clean => commands::clean::run().await.map_err(Into::into),
        Command::Logs {
            index,
            service,
            no_follow,
        } => commands::logs::run(index, service, no_follow)
            .await
            .map_err(Into::into),
        Command::Status => commands::status::run().await.map_err(Into::into),
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "rft", &mut std::io::stdout());
            Ok(())
        }
        Command::Mcp => mcp::server::run_mcp_server().await.map_err(Into::into),
    }
}
