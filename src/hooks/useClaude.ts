import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { ChatMessage, ActivityItem } from "../types";

export function useClaude() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [activities, setActivities] = useState<ActivityItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [streamingText, setStreamingText] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [error, setError] = useState<string | null>(null);
  const unlistenRefs = useRef<UnlistenFn[]>([]);

  useEffect(() => {
    const setup = async () => {
      const unlistens: UnlistenFn[] = [];

      // Listen for chat messages
      unlistens.push(
        await listen<ChatMessage>("claude:message", (event) => {
          setMessages((prev) => {
            // Deduplicate: if last message is from assistant and new one is too,
            // replace it (happens when result message duplicates streaming)
            const last = prev[prev.length - 1];
            if (
              last &&
              last.role === "assistant" &&
              event.payload.role === "assistant"
            ) {
              return [...prev.slice(0, -1), event.payload];
            }
            return [...prev, event.payload];
          });
          setStreamingText("");
        })
      );

      // Listen for text deltas (streaming)
      unlistens.push(
        await listen<string>("claude:text_delta", (event) => {
          setStreamingText((prev) => prev + event.payload);
        })
      );

      // Listen for activity events
      unlistens.push(
        await listen<ActivityItem>("claude:activity", (event) => {
          setActivities((prev) => [...prev, event.payload]);
        })
      );

      // Listen for activity completion
      unlistens.push(
        await listen<ActivityItem>("claude:activity_done", (event) => {
          setActivities((prev) =>
            prev.map((a) =>
              a.id === event.payload.id ? { ...a, status: "done" as const } : a
            )
          );
        })
      );

      // Listen for completion
      unlistens.push(
        await listen<boolean>("claude:done", (_event) => {
          setIsLoading(false);
          setStreamingText("");
        })
      );

      // Listen for errors
      unlistens.push(
        await listen<string>("claude:stderr", (event) => {
          console.warn("Claude stderr:", event.payload);
        })
      );

      unlistenRefs.current = unlistens;
    };

    setup();

    return () => {
      unlistenRefs.current.forEach((fn) => fn());
    };
  }, []);

  const sendMessage = useCallback(
    async (message: string) => {
      if (!message.trim() || isLoading) return;

      setIsLoading(true);
      setError(null);
      setStreamingText("");

      try {
        await invoke("send_message", { message });
      } catch (e) {
        setError(String(e));
        setIsLoading(false);
      }
    },
    [isLoading]
  );

  const changeWorkingDir = useCallback(async (path: string) => {
    try {
      await invoke("set_working_directory", { path });
      setWorkingDir(path);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const clearMessages = useCallback(() => {
    setMessages([]);
    setActivities([]);
    setStreamingText("");
  }, []);

  return {
    messages,
    activities,
    isLoading,
    streamingText,
    workingDir,
    error,
    sendMessage,
    changeWorkingDir,
    clearMessages,
  };
}
