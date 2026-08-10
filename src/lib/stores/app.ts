import { writable } from "svelte/store";
import type { WorkspaceProfile } from "$lib/types";

export const workspaces = writable<WorkspaceProfile[]>([]);
