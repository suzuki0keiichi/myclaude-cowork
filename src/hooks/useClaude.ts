import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { ChatMessage, ActivityItem, ApprovalRequest } from "../types";

export function useClaude() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [activities, setActivities] = useState<ActivityItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [streamingText, setStreamingText] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [lastWorkingDir, setLastWorkingDir] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pendingApproval, setPendingApproval] = useState<ApprovalRequest | null>(null);
  const initialLoadDone = useRef(false);

  // Restore saved messages and last working dir on mount
  useEffect(() => {
    invoke<ChatMessage[]>("chat_load_messages")
      .then((saved) => {
        if (saved.length > 0) setMessages(saved);
        initialLoadDone.current = true;
      })
      .catch((e) => {
        console.error("Failed to load chat history:", e);
        initialLoadDone.current = true;
      });

    invoke<string>("get_last_working_dir")
      .then((dir) => {
        if (dir) setLastWorkingDir(dir);
      })
      .catch(console.error);
  }, []);

  // Save messages to disk on change (debounced)
  useEffect(() => {
    if (!initialLoadDone.current) return;
    if (messages.length === 0) return;
    const timer = setTimeout(() => {
      invoke("chat_save_messages", { messages }).catch(console.error);
    }, 500);
    return () => clearTimeout(timer);
  }, [messages]);

  useEffect(() => {
    let active = true;
    const unlistens: UnlistenFn[] = [];

    const setup = async () => {
      // Listen for chat messages
      unlistens.push(
        await listen<ChatMessage>("claude:message", (event) => {
          if (!active) return;
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
          if (!active) return;
          setStreamingText((prev) => prev + event.payload);
        })
      );

      // Listen for activity events
      unlistens.push(
        await listen<ActivityItem>("claude:activity", (event) => {
          if (!active) return;
          setActivities((prev) => [...prev, event.payload]);
        })
      );

      // Listen for activity completion
      unlistens.push(
        await listen<ActivityItem>("claude:activity_done", (event) => {
          if (!active) return;
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
          if (!active) return;
          setIsLoading(false);
          setStreamingText("");
        })
      );

      // Listen for errors
      unlistens.push(
        await listen<string>("claude:stderr", (event) => {
          if (!active) return;
          console.warn("Claude stderr:", event.payload);
        })
      );

      // Listen for approval requests
      unlistens.push(
        await listen<ApprovalRequest>("claude:approval_request", (event) => {
          if (!active) return;
          setPendingApproval(event.payload);
        })
      );

      // If cleanup already ran while we were setting up, unregister everything
      if (!active) {
        unlistens.forEach((fn) => fn());
      }
    };

    setup();

    return () => {
      active = false;
      unlistens.forEach((fn) => fn());
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

  const respondToApproval = useCallback(async (approved: boolean) => {
    if (!pendingApproval) return;
    try {
      await invoke("respond_to_approval", {
        approvalId: pendingApproval.id,
        approved,
      });
    } catch (e) {
      console.error("Failed to respond to approval:", e);
    }
    setPendingApproval(null);
  }, [pendingApproval]);

  const cancelMessage = useCallback(async () => {
    try {
      await invoke("cancel_message");
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
    setIsLoading(false);
    setStreamingText("");
  }, []);

  const clearMessages = useCallback(() => {
    setMessages([]);
    setActivities([]);
    setStreamingText("");
    invoke("chat_clear_messages").catch(console.error);
    invoke("reset_session").catch(console.error);
  }, []);

  return {
    messages,
    activities,
    isLoading,
    streamingText,
    workingDir,
    lastWorkingDir,
    error,
    pendingApproval,
    sendMessage,
    cancelMessage,
    changeWorkingDir,
    clearMessages,
    respondToApproval,
  };
}
