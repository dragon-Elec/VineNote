import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Source } from "../types";

interface SourcesState {
  sources: Source[];
  loading: boolean;
  selectedSourceId: string | null;
  // actions
  fetchSources: () => Promise<void>;
  addSource: (
    name: string,
    source_type: string,
    url?: string,
    config?: string
  ) => Promise<Source>;
  deleteSource: (id: string) => Promise<void>;
  toggleActive: (id: string, active: boolean) => Promise<void>;
  collectSource: (id: string) => Promise<number>;
  collectAll: () => Promise<number>;
  setSelectedSourceId: (id: string | null) => void;
}

export const useSourcesStore = create<SourcesState>((set, get) => ({
  sources: [],
  loading: false,
  selectedSourceId: null,

  fetchSources: async () => {
    set({ loading: true });
    try {
      const sources: Source[] = await invoke("list_sources");
      set({ sources, loading: false });
    } catch (e) {
      console.error("fetchSources:", e);
      set({ loading: false });
    }
  },

  addSource: async (name, source_type, url, config) => {
    const source: Source = await invoke("add_source", {
      name,
      sourceType: source_type,
      url: url ?? null,
      config: config ?? null,
    });
    set((state) => ({ sources: [source, ...state.sources] }));
    return source;
  },

  deleteSource: async (id) => {
    await invoke("delete_source", { id });
    set((state) => ({
      sources: state.sources.filter((s) => s.id !== id),
      selectedSourceId:
        state.selectedSourceId === id ? null : state.selectedSourceId,
    }));
  },

  toggleActive: async (id, active) => {
    await invoke("toggle_source_active", { id, active });
    set((state) => ({
      sources: state.sources.map((s) =>
        s.id === id ? { ...s, active } : s
      ),
    }));
  },

  collectSource: async (id) => {
    const count: number = await invoke("collect_source", { id });
    // Refresh last_fetch by re-fetching
    await get().fetchSources();
    return count;
  },

  collectAll: async () => {
    const count: number = await invoke("collect_all_sources");
    await get().fetchSources();
    return count;
  },

  setSelectedSourceId: (id) => set({ selectedSourceId: id }),
}));
