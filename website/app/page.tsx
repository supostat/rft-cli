import Link from "next/link";
import { Terminal, Zap, Plug } from "lucide-react";

export default function HomePage() {
  return (
    <main className="flex flex-1 flex-col items-center justify-center px-4 text-center">
      <div className="max-w-2xl space-y-6 py-20">
        <div className="flex justify-center">
          <Terminal className="size-16 text-fd-primary" />
        </div>
        <h1 className="text-4xl font-bold tracking-tight sm:text-5xl">rft</h1>
        <p className="text-lg text-fd-muted-foreground">
          Zero-config Docker Compose isolation for git worktrees. Automatic port
          allocation, parallel stack management, and AI integration.
        </p>
        <div className="rounded-lg border border-fd-border bg-fd-card p-4">
          <code className="text-sm text-fd-foreground">
            cargo install rft-cli
          </code>
        </div>
        <div className="flex flex-col gap-3 sm:flex-row sm:justify-center">
          <Link
            href="/docs"
            className="inline-flex items-center justify-center rounded-lg bg-fd-primary px-6 py-3 text-sm font-medium text-fd-primary-foreground transition-colors hover:bg-fd-primary/90"
          >
            Get Started
          </Link>
          <Link
            href="https://github.com/nicktretyakov/rft-cli"
            className="inline-flex items-center justify-center rounded-lg border border-fd-border px-6 py-3 text-sm font-medium transition-colors hover:bg-fd-accent"
          >
            View on GitHub
          </Link>
        </div>
        <div className="grid grid-cols-1 gap-4 pt-10 sm:grid-cols-3">
          <div className="rounded-lg border border-fd-border p-6">
            <Terminal className="mx-auto mb-3 size-8 text-fd-primary" />
            <h3 className="font-semibold">Port Isolation</h3>
            <p className="mt-2 text-sm text-fd-muted-foreground">
              Each worktree gets unique ports automatically. No conflicts, no
              manual configuration.
            </p>
          </div>
          <div className="rounded-lg border border-fd-border p-6">
            <Zap className="mx-auto mb-3 size-8 text-fd-primary" />
            <h3 className="font-semibold">Parallel Start</h3>
            <p className="mt-2 text-sm text-fd-muted-foreground">
              Start all worktree stacks simultaneously. Stop waiting for
              sequential builds.
            </p>
          </div>
          <div className="rounded-lg border border-fd-border p-6">
            <Plug className="mx-auto mb-3 size-8 text-fd-primary" />
            <h3 className="font-semibold">MCP Integration</h3>
            <p className="mt-2 text-sm text-fd-muted-foreground">
              Built-in MCP server for AI agents. Manage worktrees from Claude
              Code directly.
            </p>
          </div>
        </div>
      </div>
    </main>
  );
}
