import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface PtyTab {
  id: string;
  cwd: string;
  isActive: boolean; // true if process is alive
  /** timestamp of last output, used to show the activity dot */
  lastActivityAt: number;
}

interface TerminalState {
  tabs: PtyTab[];
  activeTabId: string | null;
  // actions
  createTab: (cwd?: string) => Promise<string>;
  closeTab: (id: string) => Promise<void>;
  setActiveTab: (id: string) => void;
  markActivity: (id: string) => void;
  markExited: (id: string) => void;
}

let tabCounter = 0;

export const useTerminalStore = create<TerminalState>((set, get) => ({
  tabs: [],
  activeTabId: null,

  createTab: async (cwd?: string) => {
    const id = `pty-${++tabCounter}-${Date.now()}`;
    const tab: PtyTab = {
      id,
      cwd: cwd ?? "~",
      isActive: true,
      lastActivityAt: Date.now(),
    };

    set((s) => ({ tabs: [...s.tabs, tab], activeTabId: id }));

    // Spawn the PTY on the Rust side; rows/cols will be resized by xterm fit addon immediately after
    await invoke("create_terminal", { id, rows: 24, cols: 80, cwd: null });

    // Listen for shell exit
    const unlisten = await listen(`terminal-exit:${id}`, () => {
      get().markExited(id);
      unlisten();
    });

    return id;
  },

  closeTab: async (id: string) => {
    await invoke("kill_terminal", { id }).catch(() => {});
    set((s) => {
      const remaining = s.tabs.filter((t) => t.id !== id);
      const newActive =
        s.activeTabId === id
          ? remaining.length > 0
            ? remaining[remaining.length - 1].id
            : null
          : s.activeTabId;
      return { tabs: remaining, activeTabId: newActive };
    });
  },

  setActiveTab: (id) => set({ activeTabId: id }),

  markActivity: (id) =>
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id ? { ...t, lastActivityAt: Date.now() } : t
      ),
    })),

  markExited: (id) =>
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, isActive: false } : t)),
    })),
}));
