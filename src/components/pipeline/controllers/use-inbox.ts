import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { InboxItem, InboxBadges } from "../types";

interface InboxState {
  items: InboxItem[];
  badges: InboxBadges;
  loading: boolean;
  selectedItemId: string | null;
  statusFilter: string | null; // null = all
  // actions
  fetchItems: () => Promise<void>;
  fetchBadges: () => Promise<void>;
  updateStatus: (id: string, status: string) => Promise<void>;
  getContent: (id: string) => Promise<string>;
  addManual: (
    title: string,
    content: string,
    url?: string,
    sourceName?: string
  ) => Promise<InboxItem>;
  setSelectedItemId: (id: string | null) => void;
  setStatusFilter: (filter: string | null) => Promise<void>;
  startListening: () => Promise<() => void>;
}

export const useInboxStore = create<InboxState>((set, get) => ({
  items: [],
  badges: { unread: 0, total: 0 },
  loading: false,
  selectedItemId: null,
  statusFilter: null,

  fetchItems: async () => {
    set({ loading: true });
    try {
      const items: InboxItem[] = await invoke("list_inbox_items", {
        status: get().statusFilter,
      });
      set({ items, loading: false });
    } catch (e) {
      console.error("fetchItems:", e);
      set({ loading: false });
    }
  },

  fetchBadges: async () => {
    try {
      const badges: InboxBadges = await invoke("get_inbox_badges");
      set({ badges });
    } catch (e) {
      console.error("fetchBadges:", e);
    }
  },

  updateStatus: async (id, status) => {
    await invoke("update_inbox_item_status", { id, status });
    set((state) => ({
      items: state.items.map((item) =>
        item.id === id ? { ...item, status: status as InboxItem["status"] } : item
      ),
    }));
    await get().fetchBadges();
  },

  getContent: async (id) => {
    return await invoke<string>("get_inbox_item_content", { id });
  },

  addManual: async (title, content, url, sourceName) => {
    const item: InboxItem = await invoke("add_inbox_item_manual", {
      title,
      content,
      url: url ?? null,
      sourceName: sourceName ?? null,
    });
    set((state) => ({ items: [item, ...state.items] }));
    await get().fetchBadges();
    return item;
  },

  setSelectedItemId: (id) => set({ selectedItemId: id }),

  setStatusFilter: async (filter) => {
    set({ statusFilter: filter });
    await get().fetchItems();
  },

  startListening: async () => {
    // Listen for inbox-updated events emitted by Rust collector
    const unlisten = await listen("inbox-updated", async () => {
      await get().fetchItems();
      await get().fetchBadges();
    });
    return unlisten;
  },
}));
