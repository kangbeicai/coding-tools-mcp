import type {
  ActivityEvent,
  ActivityFilters,
  ActivitySnapshot,
  ActivityTrace,
} from "$lib/types";
import { invokeCommand } from "./transport";

export function listActivity(filters: ActivityFilters = {}): Promise<ActivitySnapshot> {
  return invokeCommand<ActivitySnapshot>("list_activity", filters as Record<string, unknown>);
}

export function getActivity(traceId: string): Promise<ActivityTrace | null> {
  return invokeCommand<ActivityTrace | null>("get_activity", { traceId });
}

export function subscribeActivity(
  onEvent: (event: ActivityEvent) => void,
  onError?: () => void,
): EventSource {
  const source = new EventSource("/api/activity/events");
  const receive = (raw: Event) => {
    const message = raw as MessageEvent<string>;
    try {
      onEvent(JSON.parse(message.data) as ActivityEvent);
    } catch {
      // A malformed observability event must not break the console.
    }
  };
  for (const kind of ["activity.started", "activity.completed", "activity.updated"]) {
    source.addEventListener(kind, receive);
  }
  if (onError) source.addEventListener("error", onError);
  return source;
}
