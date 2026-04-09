"use client";

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, RefreshCw, Trash2, ToggleLeft, ToggleRight, Rss, Link } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { useSourcesStore } from "../controllers/use-sources";
import { SOURCE_TYPE_LABELS } from "../types";
import type { Source } from "../types";

export default function SourcesPanel() {
  const { t } = useTranslation();
  const {
    sources,
    loading,
    selectedSourceId,
    fetchSources,
    addSource,
    deleteSource,
    toggleActive,
    collectSource,
    collectAll,
    setSelectedSourceId,
  } = useSourcesStore();

  const [showAdd, setShowAdd] = useState(false);
  const [newName, setNewName] = useState("");
  const [newType, setNewType] = useState<Source["source_type"]>("rss");
  const [newUrl, setNewUrl] = useState("");
  const [newConfig, setNewConfig] = useState("");
  const [collecting, setCollecting] = useState<string | null>(null);
  const [collectingAll, setCollectingAll] = useState(false);

  useEffect(() => {
    fetchSources();
  }, [fetchSources]);

  const handleAdd = async () => {
    if (!newName.trim()) return;
    try {
      await addSource(newName.trim(), newType, newUrl.trim() || undefined, newConfig.trim() || undefined);
      setNewName("");
      setNewUrl("");
      setNewConfig("");
      setShowAdd(false);
    } catch (e) {
      console.error(e);
    }
  };

  const handleCollect = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setCollecting(id);
    try {
      await collectSource(id);
    } finally {
      setCollecting(null);
    }
  };

  const handleCollectAll = async () => {
    setCollectingAll(true);
    try {
      await collectAll();
    } finally {
      setCollectingAll(false);
    }
  };

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    await deleteSource(id);
  };

  const handleToggle = async (source: Source, e: React.MouseEvent) => {
    e.stopPropagation();
    await toggleActive(source.id, !source.active);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 h-11 border-b shrink-0">
        <span className="text-sm font-medium text-foreground">{t("sources")}</span>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            title={t("collectAll")}
            onClick={handleCollectAll}
            disabled={collectingAll}
          >
            <RefreshCw
              size={13}
              className={cn(collectingAll && "animate-spin")}
            />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={() => setShowAdd((v) => !v)}
          >
            <Plus size={13} />
          </Button>
        </div>
      </div>

      {/* Add source form */}
      {showAdd && (
        <div className="p-3 border-b space-y-2 bg-muted/30 shrink-0">
          <Input
            autoFocus
            placeholder={t("sourceName")}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            className="h-7 text-xs"
          />
          <Select
            value={newType}
            onValueChange={(v) => setNewType(v as Source["source_type"])}
          >
            <SelectTrigger className="h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {(Object.keys(SOURCE_TYPE_LABELS) as Source["source_type"][]).map(
                (type) => (
                  <SelectItem key={type} value={type}>
                    <span className="flex items-center gap-1.5">
                      {(() => { const { Icon } = SOURCE_TYPE_LABELS[type]; return <Icon size={12} />; })()}
                      {SOURCE_TYPE_LABELS[type].label}
                    </span>
                  </SelectItem>
                )
              )}
            </SelectContent>
          </Select>
          {(newType === "rss" || newType === "podcast") && (
            <Input
              placeholder="https://..."
              value={newUrl}
              onChange={(e) => setNewUrl(e.target.value)}
              className="h-7 text-xs"
            />
          )}
          {newType === "youtube" && (
            <div className="space-y-1">
              <Input
                placeholder={t("youtubeUrlPlaceholder")}
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="h-7 text-xs"
              />
              <p className="text-[10px] text-muted-foreground px-0.5">
                {t("youtubeUrlHint")}
              </p>
            </div>
          )}
          {newType === "bilibili" && (
            <div className="space-y-1">
              <Input
                placeholder={t("bilibiliUrlPlaceholder")}
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="h-7 text-xs"
              />
              <p className="text-[10px] text-muted-foreground px-0.5">
                {t("bilibiliUrlHint")}
              </p>
            </div>
          )}
          {newType === "reddit" && (
            <div className="space-y-1">
              <Input
                placeholder={t("redditUrlPlaceholder")}
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="h-7 text-xs"
              />
              <p className="text-[10px] text-muted-foreground px-0.5">
                {t("redditUrlHint")}
              </p>
            </div>
          )}
          {newType === "twitter" && (
            <div className="space-y-1">
              <Input
                placeholder={t("twitterUrlPlaceholder")}
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="h-7 text-xs"
              />
              <Input
                placeholder={t("twitterConfigPlaceholder")}
                value={newConfig}
                onChange={(e) => setNewConfig(e.target.value)}
                className="h-7 text-xs font-mono"
              />
              <p className="text-[10px] text-muted-foreground px-0.5">
                {t("twitterUrlHint")}
              </p>
            </div>
          )}
          {newType === "bookmark" && (
            <div className="space-y-1">
              <Input
                placeholder={t("bookmarkPathPlaceholder")}
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="h-7 text-xs"
              />
              <p className="text-[10px] text-muted-foreground px-0.5">
                {t("bookmarkPathHint")}
              </p>
            </div>
          )}
          <div className="flex gap-2">
            <Button
              size="sm"
              className="flex-1 h-7 text-xs"
              onClick={handleAdd}
              disabled={!newName.trim()}
            >
              {t("add")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7 text-xs"
              onClick={() => setShowAdd(false)}
            >
              {t("cancel")}
            </Button>
          </div>
        </div>
      )}

      {/* Sources list */}
      <div className="flex-1 overflow-y-auto">
        {loading && sources.length === 0 && (
          <div className="flex items-center justify-center h-20 text-xs text-muted-foreground">
            {t("loading")}
          </div>
        )}
        {!loading && sources.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-2 h-32 text-center px-4">
            <Rss size={24} className="text-muted-foreground/40" />
            <p className="text-xs text-muted-foreground">{t("sourcesEmpty")}</p>
          </div>
        )}
        {sources.map((source) => {
          const typeInfo = SOURCE_TYPE_LABELS[source.source_type] ?? {
            Icon: Link,
            label: source.source_type,
          };
          const isSelected = selectedSourceId === source.id;
          return (
            <div
              key={source.id}
              className={cn(
                "group flex items-center gap-2 px-3 py-2.5 cursor-pointer border-b border-border/50 hover:bg-accent/50 transition-colors",
                isSelected && "bg-accent",
                !source.active && "opacity-50"
              )}
              onClick={() => setSelectedSourceId(source.id)}
            >
              <typeInfo.Icon size={14} className="shrink-0 text-muted-foreground" />
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium truncate">{source.name}</div>
                {source.last_fetch && (
                  <div className="text-[10px] text-muted-foreground mt-0.5">
                    {formatRelativeTime(source.last_fetch)}
                  </div>
                )}
              </div>
              {/* Action buttons — shown on hover */}
              <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-5 w-5"
                  title={source.active ? t("disable") : t("enable")}
                  onClick={(e) => handleToggle(source, e)}
                >
                  {source.active ? (
                    <ToggleRight size={12} className="text-primary" />
                  ) : (
                    <ToggleLeft size={12} className="text-muted-foreground" />
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-5 w-5"
                  title={t("collect")}
                  onClick={(e) => handleCollect(source.id, e)}
                  disabled={collecting === source.id}
                >
                  <RefreshCw
                    size={11}
                    className={cn(collecting === source.id && "animate-spin")}
                  />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-5 w-5 hover:text-destructive"
                  title={t("delete")}
                  onClick={(e) => handleDelete(source.id, e)}
                >
                  <Trash2 size={11} />
                </Button>
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
