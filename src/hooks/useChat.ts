import { useState, useEffect, useCallback } from "react";
import { api } from "../api";
import type { Message, ModelInfo, Session, SessionSummary } from "../types";

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
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string>("");

  // Fetch available models once on mount
  useEffect(() => {
    api
      .getModels()
      .then((list) => {
        setModels(list);
        // Set default to first available model if nothing selected
        setSelectedModelId((prev) => {
          if (prev) return prev;
          return list.find((m) => m.isAvailable)?.id ?? "";
        });
      })
      .catch((e) => console.error("Failed to load models:", e));
  }, []);

  // Load session when sessionId changes and reset transient state
  useEffect(() => {
    setLoading(false);
    setError(null);
    if (!sessionId) {
      setSession(null);
      return;
    }
    api
      .getSession(sessionId)
      .then((s) => {
        setSession(s);
        // Pre-select the model used in the last assistant message
        const lastModelMsg = [...s.messages]
          .reverse()
          .find((m) => m.role === "assistant" && m.modelId);
        if (lastModelMsg?.modelId) {
          setSelectedModelId(lastModelMsg.modelId);
        }
      })
      .catch((e) => console.error("Failed to load session:", e));
  }, [sessionId]);

  const sendMessage = useCallback(
    async (content: string) => {
      if (loading || !selectedModelId) return;
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
          modelId: selectedModelId,
        });

        // If new session, notify parent
        if (!sessionId) {
          const summary: SessionSummary = {
            id: response.sessionId,
            title: response.sessionTitle,
            modelName: selectedModelId,
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
    [sessionId, loading, selectedModelId, onSessionCreated, onSessionUpdated]
  );

  return { session, loading, error, sendMessage, models, selectedModelId, setSelectedModelId };
}
