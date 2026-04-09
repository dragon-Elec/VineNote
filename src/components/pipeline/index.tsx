"use client";

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Archive, Check, ExternalLink, Rss, Inbox, Sparkles, Link2, Link } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useSourcesStore } from "./controllers/use-sources";
import { useInboxStore } from "./controllers/use-inbox";
import { useCardsStore } from "./controllers/use-cards";
import { SOURCE_TYPE_LABELS } from "./types";
import type { CardConnection } from "./types";
import { useSelectedNav } from "@/components/navigation-bar/controllers/selected-nav";

export default function PipelineDetail() {
  const { t } = useTranslation();
  const selectedNav = useSelectedNav((s) => s.selectedNav);
  const { sources, selectedSourceId } = useSourcesStore();
  const { items, selectedItemId, getContent, updateStatus } = useInboxStore();
  const { cards, selectedCardId, getConnections } = useCardsStore();
  const [itemContent, setItemContent] = useState<string>("");
  const [contentLoading, setContentLoading] = useState(false);
  const [connections, setConnections] = useState<CardConnection[]>([]);

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

  // Clear stale connections when the selected card changes
  useEffect(() => {
    setConnections([]);
  }, [selectedCardId]);

  // ── Sources detail ────────────────────────────────────────────────────────
  if (selectedNav === "sources") {
    if (!selectedSource) {
      return <PipelineEmptyState icon={<Rss />} message={t("sourceSelectPrompt")} />;
    }

    const typeInfo = SOURCE_TYPE_LABELS[selectedSource.source_type] ?? {
      Icon: Link,
      label: selectedSource.source_type,
    };

    return (
      <div className="flex-1 flex flex-col p-6 overflow-y-auto">
        <div className="flex items-center gap-3 mb-6">
          <typeInfo.Icon size={20} className="text-muted-foreground shrink-0" />
          <div>
            <h2 className="text-lg font-semibold">{selectedSource.name}</h2>
            <span className="text-xs text-muted-foreground">{typeInfo.label}</span>
          </div>
          <div
            className={cn(
              "ml-auto px-2 py-0.5 rounded-full text-[11px] font-medium",
              selectedSource.active
                ? "bg-primary/10 text-primary dark:bg-primary/20"
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

  // ── Cards detail ──────────────────────────────────────────────────────────
  if (selectedNav === "cards") {
    const selectedCard = cards.find((c) => c.id === selectedCardId);

    if (!selectedCard) {
      return <PipelineEmptyState icon={<Sparkles />} message={t("cardSelectPrompt")} />;
    }

    return (
      <CardDetail
        card={selectedCard}
        connections={connections}
        onConnectionsLoad={async (id) => {
          const conns = await getConnections(id);
          setConnections(conns);
        }}
      />
    );
  }

  return null;
}

// ── Card Detail ───────────────────────────────────────────────────────────────

function CardDetail({
  card,
  connections,
  onConnectionsLoad,
}: {
  card: import("./types").Card;
  connections: CardConnection[];
  onConnectionsLoad: (id: string) => Promise<void>;
}) {
  const { t } = useTranslation();

  useEffect(() => {
    onConnectionsLoad(card.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [card.id]);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="px-8 py-5 border-b shrink-0">
        <div className="flex items-center gap-2 mb-1">
          <Sparkles size={14} className="text-primary" />
          <span className="text-[11px] text-muted-foreground">
            {new Date(card.created_at).toLocaleString()}
          </span>
        </div>
        <h1 className="text-xl font-semibold leading-snug">{card.title}</h1>
        {card.tags.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-2">
            {card.tags.map((tag) => (
              <span
                key={tag}
                className="px-2 py-0.5 rounded-full bg-primary/10 text-primary text-[11px] font-medium"
              >
                {tag}
              </span>
            ))}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-8 py-6 space-y-6">
        {/* Summary */}
        {card.summary && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
              {t("cardSummary")}
            </h3>
            <p className="text-sm leading-relaxed">{card.summary}</p>
          </section>
        )}

        {/* Key Insights */}
        {card.key_insights.length > 0 && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
              {t("cardInsights")}
            </h3>
            <ul className="space-y-1.5">
              {card.key_insights.map((insight, i) => (
                <li key={i} className="flex gap-2 text-sm">
                  <span className="text-primary mt-0.5 shrink-0">•</span>
                  <span>{insight}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* Quotes */}
        {card.quotes.length > 0 && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
              {t("cardQuotes")}
            </h3>
            <div className="space-y-2">
              {card.quotes.map((quote, i) => (
                <blockquote
                  key={i}
                  className="border-l-2 border-primary/40 pl-3 text-sm italic text-muted-foreground"
                >
                  {quote}
                </blockquote>
              ))}
            </div>
          </section>
        )}

        {/* Action Items */}
        {card.action_items.length > 0 && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
              {t("cardActions")}
            </h3>
            <ul className="space-y-1.5">
              {card.action_items.map((action, i) => (
                <li key={i} className="flex gap-2 text-sm items-start">
                  <span className="text-muted-foreground shrink-0 mt-0.5">☐</span>
                  <span>{action}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* Related Cards */}
        {connections.length > 0 && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2 flex items-center gap-1.5">
              <Link2 size={11} />
              {t("cardRelated")}
            </h3>
            <div className="space-y-2">
              {connections.map((conn) => (
                <div
                  key={conn.related_card_id}
                  className="rounded-md border p-3 hover:bg-accent/50 transition-colors cursor-pointer"
                  onClick={() =>
                    useCardsStore.getState().setSelectedCardId(conn.related_card_id)
                  }
                >
                  <div className="text-sm font-medium">{conn.related_title}</div>
                  {conn.related_summary && (
                    <div className="text-[11px] text-muted-foreground mt-0.5 line-clamp-2">
                      {conn.related_summary}
                    </div>
                  )}
                  {conn.shared_tags.length > 0 && (
                    <div className="flex gap-1 mt-1.5 flex-wrap">
                      {conn.shared_tags.map((tag) => (
                        <span
                          key={tag}
                          className="px-1.5 py-0 rounded text-[9px] bg-muted text-muted-foreground"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
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
