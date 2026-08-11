export interface AdminAuthStatus {
  ok: boolean;
  configured: boolean;
  authenticated: boolean;
  username: string;
}

interface AuthErrorPayload {
  ok?: boolean;
  error?: string;
}

async function authRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    credentials: "same-origin",
    ...init,
    headers: {
      ...(init?.body ? { "content-type": "application/json" } : {}),
      ...(init?.headers ?? {}),
    },
  });
  const payload = (await response.json()) as T & AuthErrorPayload;
  if (!response.ok) {
    throw new Error(payload.error || `Admin auth request failed: ${response.status}`);
  }
  return payload;
}

export function getAdminAuthStatus(): Promise<AdminAuthStatus> {
  return authRequest<AdminAuthStatus>("/api/auth/status");
}

export function setupAdmin(username: string, password: string): Promise<AdminAuthStatus> {
  return authRequest<AdminAuthStatus>("/api/auth/setup", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export function loginAdmin(username: string, password: string): Promise<AdminAuthStatus> {
  return authRequest<AdminAuthStatus>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export async function logoutAdmin(): Promise<void> {
  await authRequest<{ ok: boolean }>("/api/auth/logout", { method: "POST" });
}
