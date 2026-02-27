import { useState, useEffect, useCallback } from "react";
import { api } from "../api";
import type { SessionSummary } from "../types";

export function useSessions() {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const data = await api.getSessions();
      setSessions(data);
    } catch (e) {
      console.error("Failed to load sessions:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const deleteSession = useCallback(
    async (sessionId: string) => {
      await api.deleteSession(sessionId);
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
    },
    []
  );

  const upsertSession = useCallback((summary: SessionSummary) => {
    setSessions((prev) => {
      const idx = prev.findIndex((s) => s.id === summary.id);
      if (idx >= 0) {
        const updated = [...prev];
        updated[idx] = summary;
        return updated.sort(
          (a, b) =>
            new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
        );
      }
      return [summary, ...prev];
    });
  }, []);

  return { sessions, loading, refresh, deleteSession, upsertSession };
}
