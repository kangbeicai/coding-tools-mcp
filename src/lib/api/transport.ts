interface WebRpcResponse<T> {
  ok: boolean;
  result?: T;
  error?: string;
}

export async function invokeCommand<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const response = await fetch("/api/rpc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ command, args }),
  });
  const payload = (await response.json()) as WebRpcResponse<T>;
  if (response.status === 401 && typeof window !== "undefined") {
    const next = encodeURIComponent(`${window.location.pathname}${window.location.search}`);
    window.location.assign(`/login?next=${next}`);
  }
  if (!response.ok || !payload.ok) {
    throw new Error(payload.error || `Admin API request failed: ${response.status}`);
  }
  return payload.result as T;
}
