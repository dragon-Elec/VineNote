"use client";

import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Archive, Check, Inbox } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useInboxStore } from "../controllers/use-inbox";

const STATUS_FILTERS = [
  { value: null, labelKey: "inboxFilterAll" },
  { value: "unread", labelKey: "inboxFilterUnread" },
  { value: "processed", labelKey: "inboxFilterProcessed" },
  { value: "archived", labelKey: "inboxFilterArchived" },
] as const;

const STATUS_DOT: Record<string, string> = {
  unread: "bg-primary",
  reading: "bg-amber-400 dark:bg-amber-500",
  processed: "bg-muted-foreground/40",
  archived: "bg-muted-foreground/20",
};

export default function InboxPanel() {
  const { t } = useTranslation();
  const {
    items,
    badges,
    loading,
    selectedItemId,
    statusFilter,
    fetchItems,
    fetchBadges,
    updateStatus,
    setSelectedItemId,
    setStatusFilter,
    startListening,
  } = useInboxStore();

  useEffect(() => {
    fetchItems();
    fetchBadges();
    let unlisten: (() => void) | undefined;
    startListening().then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [fetchItems, fetchBadges, startListening]);

  const handleArchive = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    await updateStatus(id, "archived");
  };

  const handleMarkProcessed = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    await updateStatus(id, "processed");
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 h-11 border-b shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-foreground">{t("inbox")}</span>
          {badges.unread > 0 && (
            <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-primary text-[9px] font-bold text-primary-foreground">
              {badges.unread > 99 ? "99+" : badges.unread}
            </span>
          )}
        </div>
      </div>

      {/* Status filter tabs */}
      <div className="flex border-b shrink-0">
        {STATUS_FILTERS.map(({ value, labelKey }) => (
          <button
            key={String(value)}
            type="button"
            className={cn(
              "flex-1 py-1.5 text-[11px] font-medium transition-colors",
              statusFilter === value
                ? "text-foreground border-b-2 border-primary"
                : "text-muted-foreground hover:text-foreground"
            )}
            onClick={() => setStatusFilter(value)}
          >
            {t(labelKey)}
          </button>
        ))}
      </div>

      {/* Items list */}
      <div className="flex-1 overflow-y-auto">
        {loading && items.length === 0 && (
          <div className="flex items-center justify-center h-20 text-xs text-muted-foreground">
            {t("loading")}
          </div>
        )}
        {!loading && items.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-2 h-32 text-center px-4">
            <Inbox size={24} className="text-muted-foreground/40" />
            <p className="text-xs text-muted-foreground">{t("inboxEmpty")}</p>
          </div>
        )}
        {items.map((item) => {
          const isSelected = selectedItemId === item.id;
          return (
            <div
              key={item.id}
              className={cn(
                "group flex items-start gap-2.5 px-3 py-3 cursor-pointer border-b border-border/50 hover:bg-accent/50 transition-colors",
                isSelected && "bg-accent"
              )}
              onClick={() => {
                setSelectedItemId(item.id);
                if (item.status === "unread") {
                  updateStatus(item.id, "reading");
                }
              }}
            >
              {/* Status dot */}
              <div className="mt-1.5 shrink-0">
                <div
                  className={cn(
                    "w-1.5 h-1.5 rounded-full",
                    STATUS_DOT[item.status] ?? "bg-muted-foreground"
                  )}
                />
              </div>

              <div className="flex-1 min-w-0">
                <div
                  className={cn(
                    "text-xs leading-snug",
                    item.status === "unread" ? "font-semibold" : "font-normal"
                  )}
                >
                  {item.title || t("untitled")}
                </div>
                <div className="flex items-center gap-1.5 mt-0.5">
                  {item.source_name && (
                    <span className="text-[10px] text-muted-foreground truncate max-w-[80px]">
                      {item.source_name}
                    </span>
                  )}
                  <span className="text-muted-foreground/40 text-[10px]">·</span>
                  <span className="text-[10px] text-muted-foreground">
                    {formatRelativeTime(item.ingested_at)}
                  </span>
                  {item.word_count != null && (
                    <>
                      <span className="text-muted-foreground/40 text-[10px]">·</span>
                      <span className="text-[10px] text-muted-foreground">
                        {item.word_count}w
                      </span>
                    </>
                  )}
                </div>
              </div>

              {/* Quick action buttons */}
              <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0 mt-0.5">
                {item.status !== "processed" && (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5"
                    title={t("markProcessed")}
                    onClick={(e) => handleMarkProcessed(item.id, e)}
                  >
                    <Check size={10} className="text-primary" />
                  </Button>
                )}
                {item.status !== "archived" && (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5"
                    title={t("archive")}
                    onClick={(e) => handleArchive(item.id, e)}
                  >
                    <Archive size={10} className="text-muted-foreground" />
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function formatRelativeTime(isoString: string): string {
  try {
    const diff = Date.now() - new Date(isoString).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
  } catch {
    return "";
  }
}
