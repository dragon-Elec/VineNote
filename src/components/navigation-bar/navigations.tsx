"use client";

import { useTranslation } from "react-i18next";
import { Notebook, Tag, Settings, CircleHelp, Rss, Inbox, Sparkles, MessageSquare, SquareTerminal } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from '@/lib/utils';
import { useSelectedNav } from './controllers/selected-nav';
import { SettingsDialog } from "../settings";
import openIssue from "./controllers/open-issue";
import { useInboxStore } from "@/components/pipeline/controllers/use-inbox";
import { useEffect } from "react";
import { useRightPanel } from "@/components/right-panel/use-right-panel";
import styles from "./index.module.css";

const Navigation = function () {
  const { t } = useTranslation();
  const { selectedNav, setSelectedNav } = useSelectedNav();
  const inboxBadges = useInboxStore((s) => s.badges);
  const fetchBadges = useInboxStore((s) => s.fetchBadges);
  const { isOpen: panelOpen, activeTab: panelTab, toggle: togglePanel } = useRightPanel();

  // Fetch badge count on mount so the nav badge is accurate from the start
  useEffect(() => {
    fetchBadges();
  }, [fetchBadges]);

  const mainItems = [
    { id: "notes", name: t("notes"), Icon: Notebook },
    { id: "tags", name: t("tags"), Icon: Tag },
  ];

  const pipelineItems = [
    { id: "sources", name: t("sources"), Icon: Rss, badge: 0 },
    {
      id: "inbox",
      name: t("inbox"),
      Icon: Inbox,
      badge: inboxBadges.unread,
    },
    { id: "cards", name: t("cards"), Icon: Sparkles, badge: 0 },
  ];

  const renderNavButton = (id: string, name: string, Icon: React.ElementType, badge?: number) => {
    const isActive = selectedNav === id;
    return (
    <Button
      key={id}
      variant="ghost"
      className={cn(
        "w-full justify-start cursor-pointer relative text-[13px]",
        isActive ? "bg-accent" : ""
      )}
      onClick={() => setSelectedNav(id)}
    >
      <Icon className={cn("shrink-0", isActive ? "text-foreground" : "text-muted-foreground")} size={14} />
      <span className={cn(isActive ? "text-foreground font-medium" : "text-muted-foreground")}>{name}</span>
      {badge != null && badge > 0 && (
        <span className="ml-auto inline-flex items-center justify-center min-w-[16px] h-4 rounded-full bg-primary text-[9px] font-bold text-primary-foreground px-1">
          {badge > 99 ? "99+" : badge}
        </span>
      )}
    </Button>
  );
  };

  return (
    <div className={styles.navigation}>
      {mainItems.map(({ id, name, Icon }) => renderNavButton(id, name, Icon))}

      {/* Divider */}
      <div className="mx-3 my-1.5 border-t border-border/60" />
      <div className="px-3 mb-0.5">
        <span className="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-wider">
          {t("pipeline")}
        </span>
      </div>

      {pipelineItems.map(({ id, name, Icon, badge }) =>
        renderNavButton(id, name, Icon, badge)
      )}

      {/* Divider before tools */}
      <div className="mx-3 my-1.5 border-t border-border/60" />
      <div className="px-3 mb-0.5">
        <span className="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-wider">
          Tools
        </span>
      </div>

      {/* Chat toggle */}
      <Button
        variant="ghost"
        className={cn(
          "w-full justify-start cursor-pointer text-[13px]",
          panelOpen && panelTab === "chat" ? "bg-accent" : ""
        )}
        onClick={() => togglePanel("chat")}
      >
        <MessageSquare
          className={cn(
            "shrink-0",
            panelOpen && panelTab === "chat" ? "text-foreground" : "text-muted-foreground"
          )}
          size={14}
        />
        <span
          className={cn(
            panelOpen && panelTab === "chat" ? "text-foreground font-medium" : "text-muted-foreground"
          )}
        >
          Chat
        </span>
      </Button>

      {/* Terminal toggle */}
      <Button
        variant="ghost"
        className={cn(
          "w-full justify-start cursor-pointer text-[13px]",
          panelOpen && panelTab === "terminal" ? "bg-accent" : ""
        )}
        onClick={() => togglePanel("terminal")}
      >
        <SquareTerminal
          className={cn(
            "shrink-0",
            panelOpen && panelTab === "terminal" ? "text-foreground" : "text-muted-foreground"
          )}
          size={14}
        />
        <span
          className={cn(
            panelOpen && panelTab === "terminal" ? "text-foreground font-medium" : "text-muted-foreground"
          )}
        >
          Terminal
        </span>
      </Button>

      {/* Divider before settings */}
      <div className="mx-3 my-1.5 border-t border-border/60" />

      <SettingsDialog>
        <Button
          variant="ghost"
          className="w-full justify-start cursor-pointer text-[13px]"
        >
          <Settings className="text-muted-foreground" size={14} />
          <span className="text-muted-foreground">{t("settings")}</span>
        </Button>
      </SettingsDialog>

      <Button
        variant="ghost"
        className="w-full justify-start cursor-pointer text-[13px]"
        onClick={() => openIssue()}
      >
        <CircleHelp className="text-muted-foreground" size={14} />
        <span className="text-muted-foreground">{t("help")}</span>
      </Button>
    </div>
  );
};

export default Navigation;
