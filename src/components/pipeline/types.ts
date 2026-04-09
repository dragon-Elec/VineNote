// Shared types for the AI Pipeline (Sources, Inbox, Cards)
import { Rss, Mic, Play, FileText, Bookmark, PenLine, Video, MessageCircle, Twitter } from "lucide-react";
import type { LucideIcon } from "lucide-react";

export interface Source {
  id: string;
  source_type: "rss" | "podcast" | "youtube" | "pdf" | "bookmark" | "manual" | "bilibili" | "reddit" | "twitter";
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
  reader_status?: "pending" | "fetching" | "ready" | "card_generated";
  content_source?: "full" | "summary";
}

export interface InboxBadges {
  unread: number;
  total: number;
}

export interface Card {
  id: string;
  title: string;
  summary?: string;
  key_insights: string[];
  quotes: string[];
  tags: string[];
  action_items: string[];
  topic_keys: string[];
  topic_fingerprint?: string;
  source_items: string[];   // inbox_item ids
  file_path?: string;
  created_at: string;
}

export interface CardConnection {
  card_id: string;
  related_card_id: string;
  shared_tags: string[];
  related_title?: string;
  related_summary?: string;
}

export const SOURCE_TYPE_LABELS: Record<Source["source_type"], { Icon: LucideIcon; label: string }> = {
  rss: { Icon: Rss, label: "RSS" },
  podcast: { Icon: Mic, label: "Podcast" },
  youtube: { Icon: Play, label: "YouTube" },
  bilibili: { Icon: Video, label: "Bilibili" },
  reddit: { Icon: MessageCircle, label: "Reddit" },
  twitter: { Icon: Twitter, label: "Twitter/X" },
  pdf: { Icon: FileText, label: "PDF" },
  bookmark: { Icon: Bookmark, label: "Bookmarks" },
  manual: { Icon: PenLine, label: "Manual" },
};
