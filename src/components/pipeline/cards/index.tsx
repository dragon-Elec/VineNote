"use client";

import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useCardsStore } from "../controllers/use-cards";

export default function CardsPanel() {
  const { t } = useTranslation();
  const {
    cards,
    loading,
    selectedCardId,
    fetchCards,
    deleteCard,
    setSelectedCardId,
    startListening,
  } = useCardsStore();

  useEffect(() => {
    fetchCards();
    let unlisten: (() => void) | undefined;
    startListening().then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [fetchCards, startListening]);

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    await deleteCard(id);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 h-11 border-b shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-foreground">{t("cards")}</span>
          {cards.length > 0 && (
            <span className="text-[10px] text-muted-foreground">{cards.length}</span>
          )}
        </div>
      </div>

      {/* Cards list */}
      <div className="flex-1 overflow-y-auto">
        {loading && cards.length === 0 && (
          <div className="flex items-center justify-center h-20 text-xs text-muted-foreground">
            {t("loading")}
          </div>
        )}
        {!loading && cards.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-2 h-32 text-center px-4">
            <Sparkles size={24} className="text-muted-foreground/40" />
            <p className="text-xs text-muted-foreground">{t("cardsEmpty")}</p>
          </div>
        )}
        {cards.map((card) => {
          const isSelected = selectedCardId === card.id;
          return (
            <div
              key={card.id}
              className={cn(
                "group flex items-start gap-2.5 px-3 py-3 cursor-pointer border-b border-border/50 hover:bg-accent/50 transition-colors",
                isSelected && "bg-accent"
              )}
              onClick={() => setSelectedCardId(card.id)}
            >
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium leading-snug truncate">
                  {card.title || t("untitled")}
                </div>
                {card.summary && (
                  <div className="text-[10px] text-muted-foreground mt-0.5 line-clamp-2 leading-relaxed">
                    {card.summary}
                  </div>
                )}
                <div className="flex items-center gap-1 mt-1 flex-wrap">
                  {card.tags.slice(0, 3).map((tag) => (
                    <span
                      key={tag}
                      className="inline-block px-1.5 py-0 rounded text-[9px] bg-primary/10 text-primary font-medium"
                    >
                      {tag}
                    </span>
                  ))}
                  {card.tags.length > 3 && (
                    <span className="text-[9px] text-muted-foreground">
                      +{card.tags.length - 3}
                    </span>
                  )}
                </div>
              </div>
              <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0 mt-0.5">
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-5 w-5 hover:text-destructive"
                  title={t("delete")}
                  onClick={(e) => handleDelete(card.id, e)}
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
