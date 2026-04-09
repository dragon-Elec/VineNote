import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Card, CardConnection } from "../types";

interface CardsState {
  cards: Card[];
  loading: boolean;
  selectedCardId: string | null;
  // actions
  fetchCards: () => Promise<void>;
  deleteCard: (id: string) => Promise<void>;
  getConnections: (cardId: string) => Promise<CardConnection[]>;
  setSelectedCardId: (id: string | null) => void;
  startListening: () => Promise<() => void>;
}

export const useCardsStore = create<CardsState>((set, get) => ({
  cards: [],
  loading: false,
  selectedCardId: null,

  fetchCards: async () => {
    set({ loading: true });
    try {
      const cards: Card[] = await invoke("list_cards");
      set({ cards, loading: false });
    } catch (e) {
      console.error("fetchCards:", e);
      set({ loading: false });
    }
  },

  deleteCard: async (id) => {
    await invoke("delete_card", { id });
    set((state) => ({
      cards: state.cards.filter((c) => c.id !== id),
      selectedCardId: state.selectedCardId === id ? null : state.selectedCardId,
    }));
  },

  getConnections: async (cardId) => {
    return await invoke<CardConnection[]>("get_card_connections", { cardId });
  },

  setSelectedCardId: (id) => set({ selectedCardId: id }),

  startListening: async () => {
    const unlisten = await listen("card-added", async () => {
      await get().fetchCards();
    });
    return unlisten;
  },
}));
