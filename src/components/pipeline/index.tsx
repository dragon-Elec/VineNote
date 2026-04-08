"use client";

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Archive, Check, ExternalLink, Rss, Inbox } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useSourcesStore } from "./controllers/use-sources";
import { useInboxStore } from "./controllers/use-inbox";
import { SOURCE_TYPE_LABELS } from "./types";
import { useSelectedNav } from "@/components/navigation-bar/controllers/selected-nav";

export default function PipelineDetail() {
  const { t } = useTranslation();
  const selectedNav = useSelectedNav((s) => s.selectedNav);
  const { sources, selectedSourceId } = useSourcesStore();
  const { items, selectedItemId, getContent, updateStatus } = useInboxStore();
  const [itemContent, setItemContent] = useState<string>("");
  const [contentLoading, setContentLoading] = useState(false);

  const selectedSource = sources.find((s) => s.id === selectedSourceId);
  const selectedItem = items.find((i) => i.id === selectedItemId);

  // Load inbox item content when selected
  useEffect(() => {
    if (selectedNav === "inbox" && selectedItemId) {
      setContentLoading(true);
      getContent(selectedItemId)
        .then((content) => {
          // Strip YAML frontmatter for display
          const body = content.replace(/^---[\s\S]*?---\n/, "").trim();
          setItemContent(body);
        })
        .catch(() => setItemContent(""))
        .finally(() => setContentLoading(false));
    } else {
      setItemContent("");
    }
  }, [selectedItemId, selectedNav, getContent]);

  // ── Sources detail ────────────────────────────────────────────────────────
  if (selectedNav === "sources") {
    if (!selectedSource) {
      return <PipelineEmptyState icon={<Rss />} message={t("sourceSelectPrompt")} />;
    }

    const typeInfo = SOURCE_TYPE_LABELS[selectedSource.source_type] ?? {
      icon: "📌",
      label: selectedSource.source_type,
    };

    return (
      <div className="flex-1 flex flex-col p-6 overflow-y-auto">
        <div className="flex items-center gap-3 mb-6">
          <span className="text-3xl">{typeInfo.icon}</span>
          <div>
            <h2 className="text-lg font-semibold">{selectedSource.name}</h2>
            <span className="text-xs text-muted-foreground">{typeInfo.label}</span>
          </div>
          <div
            className={cn(
              "ml-auto px-2 py-0.5 rounded-full text-[11px] font-medium",
              selectedSource.active
                ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
                : "bg-muted text-muted-foreground"
            )}
          >
            {selectedSource.active ? t("active") : t("inactive")}
          </div>
        </div>

        <div className="space-y-4">
          {selectedSource.url && (
            <DetailRow label="URL">
              <a
                href={selectedSource.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-primary hover:underline flex items-center gap-1 truncate"
              >
                {selectedSource.url}
                <ExternalLink size={11} className="shrink-0" />
              </a>
            </DetailRow>
          )}
          <DetailRow label={t("sourceType")}>{typeInfo.label}</DetailRow>
          {selectedSource.last_fetch && (
            <DetailRow label={t("lastFetched")}>
              {new Date(selectedSource.last_fetch).toLocaleString()}
            </DetailRow>
          )}
          <DetailRow label={t("status")}>
            {selectedSource.active ? t("active") : t("inactive")}
          </DetailRow>
          <DetailRow label={t("addedOn")}>
            {new Date(selectedSource.created_at).toLocaleDateString()}
          </DetailRow>
        </div>
      </div>
    );
  }

  // ── Inbox detail ──────────────────────────────────────────────────────────
  if (selectedNav === "inbox") {
    if (!selectedItem) {
      return <PipelineEmptyState icon={<Inbox />} message={t("inboxSelectPrompt")} />;
    }

    return (
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Article header */}
        <div className="px-8 py-5 border-b shrink-0">
          <h1 className="text-xl font-semibold leading-snug mb-2">
            {selectedItem.title || t("untitled")}
          </h1>
          <div className="flex items-center gap-3 text-xs text-muted-foreground flex-wrap">
            {selectedItem.source_name && (
              <span className="font-medium">{selectedItem.source_name}</span>
            )}
            <span>{new Date(selectedItem.ingested_at).toLocaleString()}</span>
            {selectedItem.word_count != null && (
              <span>{selectedItem.word_count} words</span>
            )}
            {selectedItem.url && (
              <a
                href={selectedItem.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-0.5 text-primary hover:underline"
              >
                {t("viewOriginal")} <ExternalLink size={10} />
              </a>
            )}
          </div>

          {/* Action bar */}
          <div className="flex items-center gap-2 mt-3">
            {selectedItem.status !== "processed" && (
              <Button
                size="sm"
                variant="outline"
                className="h-7 text-xs gap-1.5"
                onClick={() => updateStatus(selectedItem.id, "processed")}
              >
                <Check size={12} className="text-green-500" />
                {t("markProcessed")}
              </Button>
            )}
            {selectedItem.status !== "archived" && (
              <Button
                size="sm"
                variant="outline"
                className="h-7 text-xs gap-1.5"
                onClick={() => updateStatus(selectedItem.id, "archived")}
              >
                <Archive size={12} />
                {t("archive")}
              </Button>
            )}
          </div>
        </div>

        {/* Article body */}
        <div className="flex-1 overflow-y-auto px-8 py-6">
          {contentLoading ? (
            <div className="text-sm text-muted-foreground animate-pulse">{t("loading")}</div>
          ) : (
            <div className="prose prose-sm dark:prose-invert max-w-none whitespace-pre-wrap leading-relaxed text-sm">
              {itemContent || <span className="text-muted-foreground">{t("noContent")}</span>}
            </div>
          )}
        </div>
      </div>
    );
  }

  return null;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function PipelineEmptyState({
  icon,
  message,
}: {
  icon: React.ReactNode;
  message: string;
}) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-3 text-muted-foreground/50">
      <div className="[&>svg]:size-10">{icon}</div>
      <p className="text-sm">{message}</p>
    </div>
  );
}

function DetailRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex gap-3">
      <span className="text-xs text-muted-foreground w-[100px] shrink-0 pt-0.5">{label}</span>
      <span className="text-sm flex-1">{children}</span>
    </div>
  );
}
