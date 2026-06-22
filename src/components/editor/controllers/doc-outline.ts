import { create } from 'zustand';

export interface IOutlineItem {
  id: string;
  title: string;
  depth: number;
}

export interface IDocOutlineState {
  headings: IOutlineItem[];
  setHeadings: (headings: IOutlineItem[]) => void;
}

export const useDocOutline = create<IDocOutlineState>(set => ({
  headings: [],
  setHeadings: (headings) => set({ headings }),
}));
