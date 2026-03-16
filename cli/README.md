# rft

Zero-config Docker Compose isolation for git worktrees.

When you work on multiple branches simultaneously using `git worktree`, each worktree needs its own Docker Compose stack with unique ports. **rft** automates this — it detects your compose file, allocates non-conflicting ports, and manages the stacks.

## Install

```bash
cargo install rft-cli
```

## Usage

```bash
# Prepare: compose file with ${VAR:-default} ports, then create worktrees
git worktree add -b feature/auth ../myproject-auth

# Manage isolated stacks
rft list       # see worktrees with unique allocated ports
rft start      # start all stacks in parallel
rft logs 1     # stream logs for worktree #1
rft stop       # stop all stacks
rft promote 1  # transfer changes to current branch
rft clean      # stop everything, remove worktrees
```

## Requirements

- Git 2.5+ (worktree support)
- Docker with Compose v2 (`docker compose`)
- Ports in compose file using `${VAR:-default}` format

## Documentation

Full docs: [github.com/supostat/rft-cli](https://github.com/supostat/rft-cli)

## License

MIT
