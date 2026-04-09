import { create } from "zustand";

export type RightPanelTab = "chat" | "terminal" | "research" | "illustrate" | string;

const SNAP_WIDTHS = [280, 320, 400, 480] as const;
const DEFAULT_WIDTH = 320;
const MIN_WIDTH = 280;
const MAX_WIDTH = 600;

function snapToNearest(w: number): number {
  let closest: number = SNAP_WIDTHS[0];
  let minDist = Math.abs(w - SNAP_WIDTHS[0]);
  for (const snap of SNAP_WIDTHS) {
    const dist = Math.abs(w - snap);
    if (dist < minDist) {
      minDist = dist;
      closest = snap;
    }
  }
  return closest;
}

interface RightPanelState {
  isOpen: boolean;
  activeTab: RightPanelTab;
  width: number;
  // actions
  open: (tab?: RightPanelTab) => void;
  close: () => void;
  toggle: (tab: RightPanelTab) => void;
  setTab: (tab: RightPanelTab) => void;
  setWidth: (w: number) => void;
  snapWidth: (w: number) => void;
  clampWidth: (w: number) => number;
}

export const useRightPanel = create<RightPanelState>((set, get) => ({
  isOpen: false,
  activeTab: "chat",
  width: DEFAULT_WIDTH,

  open: (tab) =>
    set((s) => ({ isOpen: true, activeTab: tab ?? s.activeTab })),

  close: () => set({ isOpen: false }),

  toggle: (tab) => {
    const { isOpen, activeTab } = get();
    if (isOpen && activeTab === tab) {
      set({ isOpen: false });
    } else {
      set({ isOpen: true, activeTab: tab });
    }
  },

  setTab: (tab) => set({ activeTab: tab }),

  setWidth: (w) =>
    set({ width: Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, w)) }),

  snapWidth: (w) => set({ width: snapToNearest(Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, w))) }),

  clampWidth: (w) => Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, w)),
}));
