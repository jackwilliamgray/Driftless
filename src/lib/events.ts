import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSnapshot, AuthStatus, PollError } from "./types";

export const onSnapshot = (handler: (s: AppSnapshot) => void): Promise<UnlistenFn> =>
  listen<AppSnapshot>("runs:updated", (e) => handler(e.payload));

export const onAuthChanged = (handler: (a: AuthStatus) => void): Promise<UnlistenFn> =>
  listen<AuthStatus>("auth:changed", (e) => handler(e.payload));

export const onPollError = (handler: (err: PollError) => void): Promise<UnlistenFn> =>
  listen<PollError>("poll:error", (e) => handler(e.payload));
