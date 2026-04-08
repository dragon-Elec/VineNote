// Shared types for the AI Pipeline (Sources, Inbox, Cards)

export interface Source {
  id: string;
  source_type: "rss" | "podcast" | "youtube" | "pdf" | "bookmark" | "manual";
  name: string;
  url?: string;
  config?: string;
  active: boolean;
  last_fetch?: string;
  created_at: string;
}

export interface InboxItem {
  id: string;
  source_id?: string;
  source_name?: string;
  file_path: string;
  title: string;
  url?: string;
  status: "unread" | "reading" | "processed" | "archived";
  word_count?: number;
  ingested_at: string;
}

export interface InboxBadges {
  unread: number;
  total: number;
}

export const SOURCE_TYPE_LABELS: Record<Source["source_type"], { icon: string; label: string }> = {
  rss: { icon: "📰", label: "RSS" },
  podcast: { icon: "🎙️", label: "Podcast" },
  youtube: { icon: "🎬", label: "YouTube" },
  pdf: { icon: "📄", label: "PDF" },
  bookmark: { icon: "🔖", label: "Bookmarks" },
  manual: { icon: "✏️", label: "Manual" },
};
