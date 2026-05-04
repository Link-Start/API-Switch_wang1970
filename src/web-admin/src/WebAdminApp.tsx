import React, { useEffect, useState } from 'react';
import { Power } from 'lucide-react';
import { ApiPoolPage } from '@/pages/ApiPoolPage';
import { ChannelPage } from '@/pages/ChannelPage';
import { DashboardPage } from '@/pages/DashboardPage';
import { LogPage } from '@/pages/LogPage';
import { SettingsPage } from '@/pages/SettingsPage';
import { TokenPage } from '@/pages/TokenPage';
import { MainShell, type MainPage } from '@/features/shell/MainShell';
import { clearToken, getHealth, getStatus, getToken, login, setToken, type AdminHttpError, type AdminStatus, type HealthResponse } from './api';

const GUIDE_BASE = 'https://github.com/wang1970/API-Switch/blob/master/';

function formatSeconds(seconds?: number): string {
  if (!seconds || seconds <= 0) return '稍后再试';
  if (seconds < 60) return `${seconds} 秒后再试`;
  return `${Math.ceil(seconds / 60)} 分钟后再试`;
}

function getErrorMessage(error: unknown, fallback: string): string {
  if (!error || !(error instanceof Error)) return fallback;
  const adminError = error as AdminHttpError;
  if (adminError.isRateLimitError) return `登录尝试过于频繁，请在 ${formatSeconds(adminError.retryAfterSeconds)}。`;
  if (adminError.isAuthError) return '登录已失效，请重新登录。';
  return adminError.message || fallback;
}

function LoginView({ onAuthenticated }: { onAuthenticated: () => void }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const response = await login(username, password);
      setToken(response.token);
      onAuthenticated();
    } catch (err) {
      clearToken();
      setError(getErrorMessage(err, '登录失败'));
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
            <h1 className="text-xl font-semibold">API Switch Web Admin</h1>
            <p className="mt-1 text-sm text-muted-foreground">使用 Web 管理账号登录</p>
          </div>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <label className="block space-y-1.5">
            <span className="text-sm font-medium">用户名</span>
            <input
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              autoComplete="username"
            />
          </label>
          <label className="block space-y-1.5">
            <span className="text-sm font-medium">密码</span>
            <input
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
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
            {submitting ? '登录中...' : '登录'}
          </button>
        </form>
      </div>
    </div>
  );
}

function WebMain() {
  const [currentPage, setCurrentPage] = useState<MainPage>('apiPool');
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [status, setStatus] = useState<AdminStatus | null>(null);
  const [bootstrapError, setBootstrapError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getHealth(), getStatus()])
      .then(([healthResult, statusResult]) => {
        setHealth(healthResult);
        setStatus(statusResult);
      })
      .catch((err) => {
        if (err instanceof Error && (err as AdminHttpError).isAuthError) {
          clearToken();
          window.location.reload();
          return;
        }
        setBootstrapError(getErrorMessage(err, '连接失败'));
      });
  }, []);

  const renderPage = () => {
    switch (currentPage) {
      case 'apiPool':
        return <ApiPoolPage />;
      case 'channels':
        return <ChannelPage />;
      case 'tokens':
        return <TokenPage />;
      case 'logs':
        return <LogPage />;
      case 'dashboard':
        return <DashboardPage />;
      case 'settings':
        return <SettingsPage />;
      default:
        return <ApiPoolPage />;
    }
  };

  if (bootstrapError) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background px-4">
        <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-sm">
          <h2 className="text-lg font-semibold">连接错误</h2>
          <p className="mt-2 text-sm text-destructive">{bootstrapError}</p>
          <button className="mt-4 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" onClick={() => window.location.reload()}>
            重新加载
          </button>
        </div>
      </div>
    );
  }

  return (
    <MainShell
      currentPage={currentPage}
      proxyStatus={{ running: health?.ok ?? false, address: '127.0.0.1', port: status?.port ?? 0 }}
      onNavigate={setCurrentPage}
      onOpenGuide={(path) => window.open(GUIDE_BASE + path, '_blank', 'noopener,noreferrer')}
      renderPage={renderPage}
    />
  );
}

export const WebAdminApp: React.FC = () => {
  const [authenticated, setAuthenticated] = useState(() => Boolean(getToken()));

  if (!authenticated) {
    return <LoginView onAuthenticated={() => setAuthenticated(true)} />;
  }

  return <WebMain />;
};
