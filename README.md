# rft

Zero-config Docker Compose isolation for git worktrees.

When you work on multiple branches simultaneously using `git worktree`, each worktree needs its own Docker Compose stack with unique ports. **rft** automates this — it detects your compose file, allocates non-conflicting ports, and manages the stacks.

## Quick Start

```bash
cargo install rft-cli
```

In a git repo with `docker-compose.yml` that uses env vars for ports:

```yaml
services:
  frontend:
    ports:
      - "${FRONTEND_PORT:-3000}:3000"
  api:
    ports:
      - "${API_PORT:-8080}:8080"
```

```bash
rft list       # show worktrees with allocated ports
rft start      # start all worktree stacks in parallel
rft stop       # stop all stacks
rft logs 1     # stream logs for worktree #1
rft promote 1  # copy changes from worktree #1 to current branch
rft clean      # stop everything, remove worktrees
```

## How It Works

1. Detects compose file and parses port mappings with `${VAR:-default}` format
2. For each git worktree, allocates unique ports: `20000 + default_port + worktree_index`
3. Copies compose file, Dockerfiles, and `.env` to the worktree
4. Injects port overrides into `.env`
5. Runs `docker compose up` with a unique project name

## Commands

| Command | Description |
|---------|-------------|
| `rft list` | Show all worktrees with ports and container status |
| `rft start [indices...]` | Start stacks (parallel) |
| `rft stop [indices...]` | Stop stacks (parallel) |
| `rft restart [indices...]` | Restart stacks |
| `rft logs <index> [service]` | Stream container logs |
| `rft promote <index>` | Transfer changes to current branch |
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
export PS1="$(rft status 2>/dev/null) $PS1"
```

## Requirements

- Git with worktree support
- Docker with Compose v2 (`docker compose`)
- Ports in compose file using `${VAR:-default}` format

## License

MIT
