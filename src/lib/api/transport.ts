import { invoke as tauriInvoke } from "@tauri-apps/api/core";

interface WebRpcResponse<T> {
  ok: boolean;
  result?: T;
  error?: string;
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function invokeCommand<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (isTauriRuntime()) {
    return tauriInvoke<T>(command, args);
  }

  const response = await fetch("/api/rpc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ command, args }),
  });
  const payload = (await response.json()) as WebRpcResponse<T>;
  if (!response.ok || !payload.ok) {
    throw new Error(payload.error || `Admin API request failed: ${response.status}`);
  }
  return payload.result as T;
}
