import { useState } from "react";
import { Power } from "lucide-react";

const ADMIN_PREFIX = "/admin";

export function LoginScreen({ onAuthenticated }: { onAuthenticated: (token: string) => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const res = await fetch(`${ADMIN_PREFIX}/login`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message || `HTTP ${res.status}`);
      }
      const data = await res.json();
      onAuthenticated(data.token);
    } catch (err) {
      setError(err instanceof Error ? err.message : "登录失败");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background px-4">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-sm">
        <div className="mb-6 flex items-center gap-3">
          <Power className="h-6 w-6 text-primary" />
          <div>
            <h1 className="text-xl font-semibold">API Switch</h1>
            <p className="mt-1 text-sm text-muted-foreground">Web Admin 登录</p>
          </div>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <label className="block space-y-1.5">
            <span className="text-sm font-medium">用户名</span>
            <input
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="username"
            />
          </label>
          <label className="block space-y-1.5">
            <span className="text-sm font-medium">密码</span>
            <input
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              type="password"
              autoComplete="current-password"
            />
          </label>
          {error && <div className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</div>}
          <button
            type="submit"
            disabled={submitting}
            className="w-full rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-60"
          >
            {submitting ? "登录中..." : "登录"}
          </button>
        </form>
      </div>
    </div>
  );
}
