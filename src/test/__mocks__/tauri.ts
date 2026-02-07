import { vi } from "vitest";

// Mock @tauri-apps/api/core
export const invoke = vi.fn();

// Mock @tauri-apps/api/event
const listeners = new Map<string, Set<(event: any) => void>>();

export const listen = vi.fn(async (event: string, handler: (event: any) => void) => {
  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  listeners.get(event)!.add(handler);
  return () => {
    listeners.get(event)?.delete(handler);
  };
});

export const emit = vi.fn(async (event: string, payload: any) => {
  const handlers = listeners.get(event);
  if (handlers) {
    handlers.forEach((h) => h({ payload }));
  }
});

// Utility for tests to simulate events
export function simulateEvent(event: string, payload: any) {
  const handlers = listeners.get(event);
  if (handlers) {
    handlers.forEach((h) => h({ payload }));
  }
}

export function clearListeners() {
  listeners.clear();
}
