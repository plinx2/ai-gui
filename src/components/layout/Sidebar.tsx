import type { SessionSummary } from "../../types";

interface SidebarProps {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  onSelectSession: (id: string) => void;
  onNewChat: () => void;
  onDeleteSession: (id: string) => void;
  onOpenSettings: () => void;
}

export default function Sidebar({
  sessions,
  activeSessionId,
  onSelectSession,
  onNewChat,
  onDeleteSession,
  onOpenSettings,
}: SidebarProps) {
  return (
    <aside className="flex flex-col w-64 h-full bg-slate-900 border-r border-slate-700 shrink-0">
      {/* New Chat button */}
      <div className="p-3">
        <button
          onClick={onNewChat}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-medium transition-colors"
        >
          <span className="text-lg leading-none">+</span>
          New Chat
        </button>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-y-auto px-2">
        {sessions.length === 0 ? (
          <p className="text-slate-500 text-xs text-center py-6">
            No conversations yet
          </p>
        ) : (
          <ul className="space-y-0.5">
            {sessions.map((session) => (
              <li key={session.id}>
                <button
                  onClick={() => onSelectSession(session.id)}
                  className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors group relative ${
                    session.id === activeSessionId
                      ? "bg-slate-700 text-white"
                      : "text-slate-300 hover:bg-slate-800 hover:text-white"
                  }`}
                >
                  <span className="block truncate pr-6">{session.title}</span>
                  <span className="block text-xs text-slate-500 mt-0.5">
                    {new Date(session.updatedAt).toLocaleDateString()}
                  </span>
                  {/* Delete button */}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onDeleteSession(session.id);
                    }}
                    className="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 text-xs px-1 transition-opacity"
                    title="Delete"
                  >
                    ✕
                  </button>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Settings button */}
      <div className="p-3 border-t border-slate-700">
        <button
          onClick={onOpenSettings}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 text-sm transition-colors"
        >
          <span>⚙</span>
          Settings
        </button>
      </div>
    </aside>
  );
}
