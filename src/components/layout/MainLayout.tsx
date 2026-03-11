import { useState, useCallback, useEffect } from "react";
import { useSessions } from "../../hooks/useSessions";
import Sidebar from "./Sidebar";
import ChatView from "../chat/ChatView";
import type { SessionSummary } from "../../types";

interface PlaybookRun {
  summary: SessionSummary;
  key: number;
}

interface MainLayoutProps {
  onOpenSettings: () => void;
  onOpenPlaybooks: () => void;
  playbookRun: PlaybookRun | null;
}

export default function MainLayout({
  onOpenSettings,
  onOpenPlaybooks,
  playbookRun,
}: MainLayoutProps) {
  const [chatKey, setChatKey] = useState(0);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const { sessions, deleteSession, upsertSession } = useSessions();

  // When a playbook run completes, add its session to the sidebar and open it
  useEffect(() => {
    if (!playbookRun) return;
    upsertSession(playbookRun.summary);
    setChatKey((k) => k + 1);
    setActiveSessionId(playbookRun.summary.id);
  }, [playbookRun]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleNewChat = useCallback(() => {
    setChatKey((k) => k + 1);
    setActiveSessionId(null);
  }, []);

  const handleSelectSession = useCallback((id: string) => {
    setActiveSessionId(id);
  }, []);

  const handleSessionCreated = useCallback(
    (summary: SessionSummary) => {
      upsertSession(summary);
      setActiveSessionId(summary.id);
    },
    [upsertSession],
  );

  const handleSessionUpdated = useCallback(
    (summary: SessionSummary) => {
      upsertSession(summary);
    },
    [upsertSession],
  );

  const handleDeleteSession = useCallback(
    async (id: string) => {
      await deleteSession(id);
      if (activeSessionId === id) {
        setActiveSessionId(null);
      }
    },
    [deleteSession, activeSessionId],
  );

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Sidebar
        sessions={sessions}
        activeSessionId={activeSessionId}
        onSelectSession={handleSelectSession}
        onNewChat={handleNewChat}
        onDeleteSession={handleDeleteSession}
        onOpenSettings={onOpenSettings}
        onOpenPlaybooks={onOpenPlaybooks}
      />
      <main className="flex-1 overflow-hidden">
        <ChatView
          key={chatKey}
          sessionId={activeSessionId}
          onSessionCreated={handleSessionCreated}
          onSessionUpdated={handleSessionUpdated}
        />
      </main>
    </div>
  );
}
