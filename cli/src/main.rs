use clap::{Parser, Subcommand};
use miette::Result;

mod commands;
mod compose;
mod config;
mod context;
mod error;
mod git;
mod ports;
mod sanitize;

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
    /// Start MCP server (stdio transport)
    Mcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    miette::set_panic_hook();
    let cli = Cli::parse();

    match cli.command {
        Command::List => commands::list::run().await,
        Command::Start { .. } => todo!(),
        Command::Stop { .. } => todo!(),
        Command::Restart { .. } => todo!(),
        Command::Promote { .. } => todo!(),
        Command::Clean => todo!(),
        Command::Logs { .. } => todo!(),
        Command::Mcp => todo!(),
    }
}
