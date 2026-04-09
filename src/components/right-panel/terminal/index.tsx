import { useEffect, useState } from "react";
import { Plus, X } from "lucide-react";
import { useTerminalStore } from "./use-terminal";
import { XtermView } from "./xterm-view";

const MAX_TABS = 8;
const ACTIVITY_TIMEOUT_MS = 500;

export function TerminalPanel() {
  const { tabs, activeTabId, createTab, closeTab, setActiveTab } =
    useTerminalStore();

  // Tick every 500ms so the activity dot correctly auto-turns-off
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), ACTIVITY_TIMEOUT_MS);
    return () => clearInterval(id);
  }, []);

  // Create the first tab on mount
  useEffect(() => {
    if (tabs.length === 0) {
      createTab();
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleNewTab = () => {
    if (tabs.length >= MAX_TABS) return;
    createTab();
  };

  const handleCloseTab = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    closeTab(id).then(() => {
      // If no tabs left, create a fresh one so the panel stays usable
      if (useTerminalStore.getState().tabs.length === 0) {
        createTab();
      }
    });
  };

  const isRecentlyActive = (lastActivityAt: number) =>
    Date.now() - lastActivityAt < ACTIVITY_TIMEOUT_MS;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        overflow: "hidden",
      }}
    >
      {/* PTY Tab Bar */}
      <div
        style={{
          display: "flex",
          flexDirection: "row",
          alignItems: "center",
          height: 28,
          minHeight: 28,
          borderBottom: "1px solid var(--border)",
          padding: "0 4px",
          gap: 2,
          overflowX: "auto",
          overflowY: "hidden",
          scrollbarWidth: "none",
        }}
      >
        {tabs.map((tab) => {
          const isActive = tab.id === activeTabId;
          const hasActivity = isRecentlyActive(tab.lastActivityAt);
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 5,
                padding: "0 8px",
                height: 22,
                borderRadius: 4,
                border: "none",
                cursor: "pointer",
                fontSize: 11,
                fontFamily: "inherit",
                whiteSpace: "nowrap",
                flexShrink: 0,
                background: isActive ? "var(--accent)" : "transparent",
                color: isActive ? "var(--accent-foreground)" : "var(--muted-foreground)",
                transition: "background 120ms, color 120ms",
              }}
            >
              {/* Activity / status dot */}
              <span
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: "50%",
                  flexShrink: 0,
                  background: !tab.isActive
                    ? "var(--muted-foreground)"
                    : hasActivity
                    ? "#4ade80"
                    : "var(--muted-foreground)",
                  opacity: !tab.isActive ? 0.4 : 1,
                  transition: "background 200ms",
                }}
              />
              <span>zsh</span>
              <span
                onClick={(e) => handleCloseTab(e, tab.id)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  width: 14,
                  height: 14,
                  borderRadius: 3,
                  opacity: 0,
                  transition: "opacity 100ms",
                  cursor: "pointer",
                }}
                className="pty-close-btn"
              >
                <X size={10} />
              </span>
            </button>
          );
        })}

        {/* New Tab button */}
        <button
          onClick={handleNewTab}
          disabled={tabs.length >= MAX_TABS}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            width: 22,
            height: 22,
            borderRadius: 4,
            border: "none",
            cursor: tabs.length >= MAX_TABS ? "not-allowed" : "pointer",
            background: "transparent",
            color: "var(--muted-foreground)",
            opacity: tabs.length >= MAX_TABS ? 0.3 : 1,
            flexShrink: 0,
          }}
        >
          <Plus size={12} />
        </button>
      </div>

      {/* xterm instances — rendered for all tabs but only visible for the active one */}
      <div style={{ flex: 1, position: "relative", overflow: "hidden" }}>
        {tabs.map((tab) => (
          <div
            key={tab.id}
            style={{
              position: "absolute",
              inset: 0,
              visibility: tab.id === activeTabId ? "visible" : "hidden",
              pointerEvents: tab.id === activeTabId ? "auto" : "none",
            }}
          >
            <XtermView tabId={tab.id} isVisible={tab.id === activeTabId} />
          </div>
        ))}
      </div>

      {/* Hover style for close buttons — injected once */}
      <style>{`
        button:hover .pty-close-btn {
          opacity: 0.7 !important;
        }
        .pty-close-btn:hover {
          opacity: 1 !important;
          background: var(--destructive) !important;
          color: white !important;
        }
      `}</style>
    </div>
  );
}
