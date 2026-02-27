import { useState, useEffect, useCallback } from "react";
import { api } from "../api";
import type { Message, Session, SessionSummary } from "../types";

interface UseChatOptions {
  sessionId: string | null;
  onSessionCreated: (summary: SessionSummary) => void;
  onSessionUpdated: (summary: SessionSummary) => void;
}

export function useChat({
  sessionId,
  onSessionCreated,
  onSessionUpdated,
}: UseChatOptions) {
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load session when sessionId changes
  useEffect(() => {
    if (!sessionId) {
      setSession(null);
      return;
    }
    api
      .getSession(sessionId)
      .then(setSession)
      .catch((e) => console.error("Failed to load session:", e));
  }, [sessionId]);

  const sendMessage = useCallback(
    async (content: string) => {
      if (loading) return;
      setLoading(true);
      setError(null);

      // Optimistically add user message to local state
      const optimisticUserMsg: Message = {
        id: `optimistic-${Date.now()}`,
        role: "user",
        content: { type: "text", text: content },
        createdAt: new Date().toISOString(),
      };

      setSession((prev) => {
        if (prev) {
          return { ...prev, messages: [...prev.messages, optimisticUserMsg] };
        }
        return null;
      });

      try {
        const response = await api.sendMessage({
          sessionId: sessionId ?? undefined,
          content,
        });

        // If new session, create it in the parent
        if (!sessionId) {
          const summary: SessionSummary = {
            id: response.sessionId,
            title: response.sessionTitle,
            modelName: "gemini-2.0-flash",
            updatedAt: new Date().toISOString(),
            totalInputTokens: 0,
            totalOutputTokens: 0,
          };
          onSessionCreated(summary);
        }

        // Load fresh session data from backend
        const freshSession = await api.getSession(response.sessionId);
        setSession(freshSession);

        const summary: SessionSummary = {
          id: freshSession.id,
          title: freshSession.title,
          modelName: freshSession.modelName,
          updatedAt: freshSession.updatedAt,
          totalInputTokens: freshSession.totalInputTokens,
          totalOutputTokens: freshSession.totalOutputTokens,
        };
        onSessionUpdated(summary);
      } catch (e) {
        setError(String(e));
        // Remove optimistic message on failure
        setSession((prev) => {
          if (prev) {
            return {
              ...prev,
              messages: prev.messages.filter(
                (m) => m.id !== optimisticUserMsg.id
              ),
            };
          }
          return null;
        });
      } finally {
        setLoading(false);
      }
    },
    [sessionId, loading, onSessionCreated, onSessionUpdated]
  );

  return { session, loading, error, sendMessage };
}
