import { useState, useCallback } from "react";
import { useSessions } from "../../hooks/useSessions";
import Sidebar from "./Sidebar";
import ChatView from "../chat/ChatView";
import type { SessionSummary } from "../../types";

interface MainLayoutProps {
  onOpenSettings: () => void;
}

export default function MainLayout({ onOpenSettings }: MainLayoutProps) {
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const { sessions, deleteSession, upsertSession } = useSessions();

  const handleNewChat = useCallback(() => {
    setActiveSessionId(null);
  }, []);

  const handleSessionCreated = useCallback(
    (summary: SessionSummary) => {
      upsertSession(summary);
      setActiveSessionId(summary.id);
    },
    [upsertSession]
  );

  const handleSessionUpdated = useCallback(
    (summary: SessionSummary) => {
      upsertSession(summary);
    },
    [upsertSession]
  );

  const handleDeleteSession = useCallback(
    async (id: string) => {
      await deleteSession(id);
      if (activeSessionId === id) {
        setActiveSessionId(null);
      }
    },
    [deleteSession, activeSessionId]
  );

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Sidebar
        sessions={sessions}
        activeSessionId={activeSessionId}
        onSelectSession={setActiveSessionId}
        onNewChat={handleNewChat}
        onDeleteSession={handleDeleteSession}
        onOpenSettings={onOpenSettings}
      />
      <main className="flex-1 overflow-hidden">
        <ChatView
          key={activeSessionId ?? "new"}
          sessionId={activeSessionId}
          onSessionCreated={handleSessionCreated}
          onSessionUpdated={handleSessionUpdated}
        />
      </main>
    </div>
  );
}
