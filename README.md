# rft

[![Crates.io](https://img.shields.io/crates/v/rft-cli)](https://crates.io/crates/rft-cli)
[![CI](https://github.com/supostat/rft-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/supostat/rft-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Zero-config Docker Compose isolation for git worktrees.

When you work on multiple branches simultaneously using `git worktree`, each worktree needs its own Docker Compose stack with unique ports. **rft** automates this — it detects your compose file, allocates non-conflicting ports, and manages the stacks.

## Quick Start

### 1. Install

```bash
cargo install rft-cli
```

### 2. Prepare your compose file

Ports **must** use the `${VAR:-default}` format:

```yaml
# docker-compose.yml
services:
  frontend:
    build: ./frontend
    ports:
      - "${FRONTEND_PORT:-3000}:3000"
  api:
    build: ./backend
    ports:
      - "${API_PORT:-8080}:8080"
```

### 3. Set up git worktrees

A git worktree is a separate directory checked out at a different branch, sharing the same repo history. The recommended approach is **bare repo + worktrees** — every branch (including main) lives in its own directory:

```bash
# Clone your project as a bare repo
mkdir ~/projects/myapp && cd ~/projects/myapp
git clone --bare git@github.com:you/myapp.git .bare

# Create a .git file (not directory!) so git commands work from this dir.
# Without it, git doesn't know where the repo is.
echo "gitdir: ./.bare" > .git

# Bare clones don't track remote branches by default.
# This tells git to fetch all branches from origin — same as a normal clone.
git config remote.origin.fetch "+refs/heads/*:refs/remotes/origin/*"

# Create a worktree for main
git worktree add main

# Create worktrees for feature branches
git worktree add feature-auth -b feature/auth
git worktree add feature-payments -b feature/payments
```

Result:

```
~/projects/myapp/
├── .bare/              ← git objects (not a working directory)
├── .git                ← file pointing to .bare
├── main/               ← main branch
├── feature-auth/       ← feature/auth branch
└── feature-payments/   ← feature/payments branch
```

Each directory has its own files. You can `cd` into any, edit, commit, and run Docker independently.

**The problem:** `docker compose up` in each directory tries to bind the same ports (3000, 8080). That's what rft solves.

> **Alternative:** you can also create worktrees from a regular (non-bare) repo with `git worktree add ../myapp-auth -b feature/auth`. rft works with both approaches.

### 4. Use rft

Run `rft` from the **main worktree** (`~/projects/myapp/main`):

```bash
cd ~/projects/myapp/main

rft list       # see worktrees with unique allocated ports
rft start      # start all stacks in parallel
rft logs 1     # stream logs for worktree #1 (feature/auth)
rft stop       # stop all stacks
rft promote 1  # transfer changes from worktree #1 to current branch
rft clean      # stop everything, remove worktrees and Docker resources
```

rft assigns unique ports to each worktree automatically:

```
┌───┬──────────────────┬────────┬────────────────────────────────────┐
│ # │ Branch           │ Status │ Ports                              │
├───┼──────────────────┼────────┼────────────────────────────────────┤
│ 1 │ feature/auth     │ down   │ FRONTEND_PORT=23001, API_PORT=28081│
│ 2 │ feature/payments │ down   │ FRONTEND_PORT=23002, API_PORT=28082│
└───┴──────────────────┴────────┴────────────────────────────────────┘
```

Main branch keeps default ports (`3000`, `8080`) — rft doesn't touch it.

## How It Works

1. Detects compose file and parses port mappings with `${VAR:-default}` format
2. For each git worktree, allocates unique ports: `20000 + default_port + worktree_index`
3. Copies compose file, Dockerfiles, and `.env` to the worktree
4. Injects port overrides into `.env`
5. Runs `docker compose up` with a unique project name

## Commands

| Command | Description |
|---------|-------------|
| `rft init` | Analyze compose file, suggest port fixes, generate `.rftrc.toml` |
| `rft list` | Show all worktrees with ports and container status |
| `rft start [indices...]` | Start stacks (parallel). `--dry-run` to preview |
| `rft stop [indices...]` | Stop stacks (parallel) |
| `rft restart [indices...]` | Restart stacks |
| `rft watch [indices...]` | Start stacks and auto-restart on file changes |
| `rft logs <index> [service]` | Stream container logs. `--no-follow` for snapshot |
| `rft promote <index>` | Transfer changes to current branch. `--dry-run` to preview |
| `rft clean` | Full cleanup: stop, remove worktrees, prune Docker |
| `rft status` | One-line status for shell prompt (`rft: 3/5 up`) |
| `rft completions <shell>` | Generate shell completions (bash/zsh/fish) |
| `rft mcp` | Start MCP server for AI agent integration |

## Configuration

Create `.rftrc.toml` in your repo root:

```toml
# Extra files to sync to worktrees
sync = ["nginx/", "scripts/init.sql"]

# Custom port offset (default: 20000)
port_offset = 30000

# Main branch name (default: auto-detects "main" or "master")
main_branch = "develop"

# Environment variable templates (${VAR} substituted with allocated port)
[env_overrides]
API_URL = "http://localhost:${API_PORT}"
```

Also supports `.rftrc.json` and `package.json` (`rft` field).

Environment variables: `RFT_PORT_OFFSET`, `RFT_SYNC` (comma-separated).

## Port Allocation

Formula: `base_offset + default_port + worktree_index`

| Worktree | FRONTEND_PORT (default 3000) | API_PORT (default 8080) |
|----------|------------------------------|-------------------------|
| #1 | 23001 | 28081 |
| #2 | 23002 | 28082 |
| #3 | 23003 | 28083 |

If the result exceeds 65535, fallback: `default_port + 100 * index`.

## Promote

`rft promote <index>` transfers changes from a worktree to your current branch:

- **Committed files** → `git checkout` (preserves git history)
- **Uncommitted/untracked files** → direct file copy
- Excludes `.env` and compose files automatically
- `--dry-run` to preview, `--files "*.rs"` to filter

## MCP Integration

Add to your Claude Code config:

```json
{
  "mcpServers": {
    "rft": {
      "command": "rft",
      "args": ["mcp"]
    }
  }
}
```

6 tools available: `rft_start`, `rft_stop`, `rft_restart`, `rft_list`, `rft_promote`, `rft_clean`.

## Shell Prompt

Add to `.zshrc` / `.bashrc`:

```bash
# Zsh: add to ~/.zshrc
BASE_PS1="$PS1"
precmd() { PS1="$(rft status 2>/dev/null) $BASE_PS1"; }

# Bash: add to ~/.bashrc
export PS1="\$(rft status 2>/dev/null) $PS1"
```

## Requirements

- Git with worktree support
- Docker with Compose v2 (`docker compose`)
- Ports in compose file using `${VAR:-default}` format

## License

MIT
