"use client";

import {
  type ReactNode,
  createContext,
  useContext,
  useState,
  useRef,
  useLayoutEffect
} from "react";


import {
  SunMoon,
  BookA,
  Type,
  Wand2,
  Brain,
  Plus,
  X,
  ChevronRight,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/plate-ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
} from "@/components/plate-ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { setTheme, type Theme } from "@/utils/set-theme";
import { setFont, type Font } from "@/utils/set-font";
import { setConfig, getConfig } from "@/utils/store-config";
import i18n from "@/i18n";
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { useAiChatConfig } from "@/lib/ai-chat-config";

export type Language = "en" | "zh";

export interface IAIModels {
  reader: string;
  writer: string;
  editor: string;
}

export const AI_PROVIDERS = [
  { id: "openai", label: "OpenAI", endpoint: "https://api.openai.com/v1" },
  { id: "anthropic", label: "Anthropic", endpoint: "https://api.anthropic.com/v1" },
  { id: "ollama", label: "Ollama", endpoint: "http://localhost:11434/v1" },
  { id: "deepseek", label: "DeepSeek", endpoint: "https://api.deepseek.com/v1" },
  { id: "gemini", label: "Gemini", endpoint: "https://generativelanguage.googleapis.com/v1beta" },
  { id: "custom", label: "Custom", endpoint: "" },
] as const;

export type AIProvider = (typeof AI_PROVIDERS)[number]["id"];

export interface IPersona {
  identity: string;
  focusAreas: string[];
  valueFilter: "actionable" | "depth" | "comprehensive";
  readingStyle: "academic" | "practitioner" | "casual";
  outputPrefs: string[];
  customPrompt: string;
}

export interface ISettings {
  theme: Theme;
  language: Language;
  font: Font;
  // AI Provider
  aiProvider: AIProvider;
  aiEndpoint: string;
  aiKey: string;
  aiModels: IAIModels;
  // Persona
  persona: IPersona;
  // Legacy (kept for backward compat with editor AI)
  model: string;
  modelApiKey: string;
  uploadThingApiKey: string;
}

const languages = [
  { label: "English", value: "en" },
  { label: "简体中文", value: "zh" },
];

const OUTPUT_PREF_OPTIONS = [
  { id: "quotes", labelKey: "personaOutputQuotes" },
  { id: "connections", labelKey: "personaOutputConnections" },
  { id: "counterintuitive", labelKey: "personaOutputCounterintuitive" },
  { id: "stats", labelKey: "personaOutputStats" },
  { id: "actions", labelKey: "personaOutputActions" },
];

export const defaultPersona: IPersona = {
  identity: "",
  focusAreas: [],
  valueFilter: "actionable",
  readingStyle: "practitioner",
  outputPrefs: ["quotes", "connections"],
  customPrompt: "",
};

export const defaultSettings: ISettings = {
  theme: "system" as Theme,
  language: (window.navigator.language === "zh-CN" ? "zh" : "en") as Language,
  font: "cangerJinkai" as Font,
  aiProvider: "openai",
  aiEndpoint: "https://api.openai.com/v1",
  aiKey: "",
  aiModels: { reader: "gpt-4o-mini", writer: "gpt-4o", editor: "gpt-4o-mini" },
  persona: defaultPersona,
  model: "",
  modelApiKey: "",
  uploadThingApiKey: "",
};

type SettingsProviderState = {
  settings: ISettings;
  setSettings: (config: ISettings) => void;
};

const initialState: SettingsProviderState = {
  settings: defaultSettings,
  setSettings: () => null,
};

export const SettingsProviderContext =
  createContext<SettingsProviderState>(initialState);

export const useSettings = () => {
  const context = useContext(SettingsProviderContext);
  if (context === undefined)
    throw new Error("useSettings must be used within a SettingsProvider");
  return context;
};

type ConfigProviderProps = { children: React.ReactNode };

export function SettingsProvider({ children }: ConfigProviderProps) {
  const [settings, setSettings] = useState<ISettings>(defaultSettings);

  useLayoutEffect(() => {
    getConfig().then((config: ISettings) => {
      if (config.theme) {
        i18n.changeLanguage(config.language);
        setTheme(config.theme);
        const merged = { ...defaultSettings, ...config };
        setSettings(merged);
        // Sync AI config to the editor's chat config store (copilot + AI menu)
        useAiChatConfig.getState().setConfig({
          endpoint: merged.aiEndpoint ?? "",
          apiKey: merged.aiKey ?? merged.modelApiKey ?? "",
          writerModel: merged.aiModels?.writer ?? "gpt-4o-mini",
        });
        // Sync persisted AI config to Rust backend so the scheduler is ready
        // immediately on startup without requiring the user to re-open Settings.
        invoke("update_ai_config", {
          endpoint: merged.aiEndpoint ?? "",
          apiKey: merged.aiKey ?? merged.modelApiKey ?? "",
          readerModel: merged.aiModels?.reader ?? "gpt-4o-mini",
          writerModel: merged.aiModels?.writer ?? "gpt-4o",
          systemPrompt: buildSystemPrompt(merged),
          collectIntervalMins: 30,
        }).catch(() => {});
      }
    });
  }, []);

  return (
    <SettingsProviderContext.Provider value={{ settings, setSettings }}>
      {children}
    </SettingsProviderContext.Provider>
  );
}

// ── Settings Dialog ───────────────────────────────────────────────────────────

function buildSystemPrompt(s: ISettings): string {
  const p = s.persona;
  const parts: string[] = [];
  if (p.identity) parts.push(p.identity);
  if (p.focusAreas.length > 0)
    parts.push(`My focus areas: ${p.focusAreas.join(", ")}.`);
  const valueMap = {
    actionable: "Prioritize actionable, practical insights.",
    depth: "Prioritize depth and rigorous analysis over breadth.",
    comprehensive: "Provide comprehensive coverage of the topic.",
  };
  parts.push(valueMap[p.valueFilter]);
  const styleMap = {
    academic: "Use an academic, analytical reading lens.",
    practitioner: "Use a practitioner, hands-on perspective.",
    casual: "Keep notes casual and exploratory.",
  };
  parts.push(styleMap[p.readingStyle]);
  if (p.customPrompt) parts.push(p.customPrompt);
  if (parts.length === 0)
    return "You are a knowledge management assistant that extracts structured insights from articles.";
  return parts.join(" ");
}

type SettingsTab = "appearance" | "ai" | "persona";

export function SettingsDialog({ children }: { children: ReactNode }) {
  const { settings, setSettings } = useSettings();
  const [open, setOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const { t } = useTranslation();

  // Ref-backed fields to avoid re-renders on every keystroke
  const aiKeyRef = useRef(settings.aiKey);
  const aiEndpointRef = useRef(settings.aiEndpoint);
  const modelsRef = useRef<IAIModels>({ ...settings.aiModels });
  const personaIdentityRef = useRef(settings.persona.identity);
  const personaCustomPromptRef = useRef(settings.persona.customPrompt);
  const [personaFocusAreas, setPersonaFocusAreas] = useState<string[]>(
    settings.persona.focusAreas
  );
  const [focusAreaInput, setFocusAreaInput] = useState("");
  const [personaValueFilter, setPersonaValueFilter] = useState(
    settings.persona.valueFilter
  );
  const [personaReadingStyle, setPersonaReadingStyle] = useState(
    settings.persona.readingStyle
  );
  const [personaOutputPrefs, setPersonaOutputPrefs] = useState<string[]>(
    settings.persona.outputPrefs
  );
  const [aiProvider, setAiProvider] = useState<AIProvider>(settings.aiProvider);

  const handleThemeChange = (value: Theme) => {
    setTheme(value);
    const next = { ...settings, theme: value };
    setSettings(next);
    setConfig(JSON.stringify(next));
  };

  const handleLanguageChange = (value: Language) => {
    i18n.changeLanguage(value);
    const next = {
      ...settings,
      language: value,
      font: value === "en" ? ("system" as Font) : settings.font,
    };
    setFont(next.font);
    setSettings(next);
    setConfig(JSON.stringify(next));
  };

  const handleFontChange = (value: Font) => {
    setFont(value);
    const next = { ...settings, font: value };
    setSettings(next);
    setConfig(JSON.stringify(next));
  };

  const handleProviderChange = (id: AIProvider) => {
    setAiProvider(id);
    const preset = AI_PROVIDERS.find((p) => p.id === id);
    if (preset && preset.endpoint) {
      aiEndpointRef.current = preset.endpoint;
    }
  };

  const handleAddFocusArea = () => {
    const tag = focusAreaInput.trim();
    if (tag && !personaFocusAreas.includes(tag)) {
      setPersonaFocusAreas((prev) => [...prev, tag]);
    }
    setFocusAreaInput("");
  };

  const handleRemoveFocusArea = (tag: string) => {
    setPersonaFocusAreas((prev) => prev.filter((t) => t !== tag));
  };

  const toggleOutputPref = (id: string) => {
    setPersonaOutputPrefs((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const next: ISettings = {
      ...settings,
      aiProvider,
      aiEndpoint: aiEndpointRef.current,
      aiKey: aiKeyRef.current,
      aiModels: { ...modelsRef.current },
      // Legacy compat
      modelApiKey: aiKeyRef.current,
      model: modelsRef.current.editor,
      persona: {
        identity: personaIdentityRef.current,
        focusAreas: personaFocusAreas,
        valueFilter: personaValueFilter,
        readingStyle: personaReadingStyle,
        outputPrefs: personaOutputPrefs,
        customPrompt: personaCustomPromptRef.current,
      },
    };
    setSettings(next);
    setConfig(JSON.stringify(next));

    // Sync AI config to the editor's chat config store
    useAiChatConfig.getState().setConfig({
      endpoint: next.aiEndpoint,
      apiKey: next.aiKey,
      writerModel: next.aiModels.writer,
    });
    // Sync AI config to Rust backend (for background scheduler)
    const systemPrompt = buildSystemPrompt(next);
    invoke("update_ai_config", {
      endpoint: next.aiEndpoint,
      apiKey: next.aiKey,
      readerModel: next.aiModels.reader,
      writerModel: next.aiModels.writer,
      systemPrompt,
      collectIntervalMins: 30,
    }).catch((e) => console.error("update_ai_config:", e));

    setOpen(false);
  };

  const tabs: { id: SettingsTab; label: string; icon: React.ReactNode }[] = [
    {
      id: "appearance",
      label: t("settingsTabAppearance"),
      icon: <SunMoon size={14} />,
    },
    {
      id: "ai",
      label: t("settingsTabAI"),
      icon: <Wand2 size={14} />,
    },
    {
      id: "persona",
      label: t("settingsTabPersona"),
      icon: <Brain size={14} />,
    },
  ];

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent className="max-w-[580px] p-0 gap-0 overflow-hidden">
        <div className="flex h-full">
          {/* Left tab nav */}
          <div className="flex flex-col w-[160px] shrink-0 border-r bg-muted/30 p-3 gap-1">
            <div className="px-2 py-2 mb-1">
              <DialogTitle className="text-base">{t("settings")}</DialogTitle>
              <DialogDescription className="text-xs mt-0.5">
                {t("configurationTip")}
              </DialogDescription>
            </div>
            {tabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "flex items-center gap-2 px-3 py-2 rounded-md text-sm text-left transition-colors",
                  activeTab === tab.id
                    ? "bg-background shadow-sm text-foreground font-medium"
                    : "text-muted-foreground hover:text-foreground hover:bg-background/60"
                )}
              >
                {tab.icon}
                {tab.label}
                {activeTab === tab.id && (
                  <ChevronRight size={12} className="ml-auto" />
                )}
              </button>
            ))}
          </div>

          {/* Right content */}
          <form
            className="flex-1 flex flex-col overflow-y-auto"
            onSubmit={handleSubmit}
          >
            <div className="flex-1 p-6 space-y-5">
              {/* ── Appearance ────────────────────── */}
              {activeTab === "appearance" && (
                <>
                  <SettingGroup
                    icon={<SunMoon size={14} className="text-blue-600 dark:text-blue-400" />}
                    iconBg="bg-blue-100 dark:bg-blue-900"
                    label={t("theme")}
                  >
                    <Select
                      defaultValue={settings.theme}
                      onValueChange={handleThemeChange}
                    >
                      <SelectTrigger className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="light">{t("lightTheme")}</SelectItem>
                        <SelectItem value="dark">{t("darkTheme")}</SelectItem>
                        <SelectItem value="system">{t("systemTheme")}</SelectItem>
                      </SelectContent>
                    </Select>
                  </SettingGroup>

                  <SettingGroup
                    icon={<BookA size={14} className="text-orange-600 dark:text-orange-400" />}
                    iconBg="bg-orange-100 dark:bg-orange-900"
                    label={t("language")}
                  >
                    <Select
                      defaultValue={settings.language}
                      onValueChange={handleLanguageChange}
                    >
                      <SelectTrigger className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {languages.map(({ label, value }) => (
                          <SelectItem key={value} value={value}>
                            {label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </SettingGroup>

                  <SettingGroup
                    icon={<Type size={14} className="text-purple-600 dark:text-purple-400" />}
                    iconBg="bg-purple-100 dark:bg-purple-900"
                    label={t("font")}
                  >
                    <Select
                      value={settings.font}
                      onValueChange={handleFontChange}
                    >
                      <SelectTrigger className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="system">{t("systemFont")}</SelectItem>
                        <SelectItem value="cangerJinkai">{t("cangerJinkai")}</SelectItem>
                        <SelectItem value="cangerXuansan">{t("cangerXuansan")}</SelectItem>
                      </SelectContent>
                    </Select>
                  </SettingGroup>
                </>
              )}

              {/* ── AI Provider ────────────────────── */}
              {activeTab === "ai" && (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("aiProvider")}</label>
                    <div className="grid grid-cols-3 gap-2">
                      {AI_PROVIDERS.map((p) => (
                        <button
                          key={p.id}
                          type="button"
                          onClick={() => handleProviderChange(p.id as AIProvider)}
                          className={cn(
                            "px-3 py-2 rounded-md border text-sm font-medium transition-colors",
                            aiProvider === p.id
                              ? "border-primary bg-primary/10 text-primary"
                              : "border-border hover:border-primary/50 text-muted-foreground"
                          )}
                        >
                          {p.label}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("aiEndpoint")}</label>
                    <Input
                      defaultValue={settings.aiEndpoint}
                      placeholder="https://api.openai.com/v1"
                      onChange={(e) => {
                        aiEndpointRef.current = e.target.value;
                      }}
                    />
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("aiKey")}</label>
                    <Input
                      type="password"
                      defaultValue={settings.aiKey}
                      placeholder="sk-..."
                      data-1p-ignore
                      onChange={(e) => {
                        aiKeyRef.current = e.target.value;
                      }}
                    />
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("aiModelAssignment")}</label>
                    <div className="space-y-2 rounded-md border p-3">
                      {(
                        [
                          { key: "reader", label: t("aiModelReader") },
                          { key: "writer", label: t("aiModelWriter") },
                          { key: "editor", label: t("aiModelEditor") },
                        ] as const
                      ).map(({ key, label }) => (
                        <div key={key} className="flex items-center gap-3">
                          <span className="text-xs text-muted-foreground w-[80px] shrink-0">
                            {label}
                          </span>
                          <Input
                            className="h-7 text-xs"
                            defaultValue={settings.aiModels[key]}
                            placeholder="model name"
                            onChange={(e) => {
                              modelsRef.current[key] = e.target.value;
                            }}
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                </>
              )}

              {/* ── Persona ────────────────────── */}
              {activeTab === "persona" && (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("personaIdentity")}</label>
                    <p className="text-xs text-muted-foreground">{t("personaIdentityTip")}</p>
                    <Input
                      defaultValue={settings.persona.identity}
                      placeholder={t("personaIdentityPlaceholder")}
                      onChange={(e) => {
                        personaIdentityRef.current = e.target.value;
                      }}
                    />
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("personaFocusAreas")}</label>
                    <div className="flex gap-2">
                      <Input
                        value={focusAreaInput}
                        placeholder={t("personaFocusAreaPlaceholder")}
                        onChange={(e) => setFocusAreaInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            handleAddFocusArea();
                          }
                        }}
                        className="flex-1"
                      />
                      <Button
                        type="button"
                        size="icon"
                        variant="outline"
                        onClick={handleAddFocusArea}
                        className="shrink-0"
                      >
                        <Plus size={14} />
                      </Button>
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      {personaFocusAreas.map((tag) => (
                        <span
                          key={tag}
                          className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full bg-primary/10 text-primary text-xs font-medium"
                        >
                          {tag}
                          <button
                            type="button"
                            onClick={() => handleRemoveFocusArea(tag)}
                            className="hover:opacity-70"
                          >
                            <X size={10} />
                          </button>
                        </span>
                      ))}
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("personaValueFilter")}</label>
                    <div className="space-y-1.5">
                      {(
                        [
                          { value: "actionable", labelKey: "personaValueActionable" },
                          { value: "depth", labelKey: "personaValueDepth" },
                          { value: "comprehensive", labelKey: "personaValueComprehensive" },
                        ] as const
                      ).map(({ value, labelKey }) => (
                        <label
                          key={value}
                          className="flex items-center gap-2.5 cursor-pointer group"
                        >
                          <div
                            className={cn(
                              "w-4 h-4 rounded-full border-2 flex items-center justify-center transition-colors",
                              personaValueFilter === value
                                ? "border-primary"
                                : "border-border group-hover:border-primary/50"
                            )}
                            onClick={() => setPersonaValueFilter(value)}
                          >
                            {personaValueFilter === value && (
                              <div className="w-2 h-2 rounded-full bg-primary" />
                            )}
                          </div>
                          <span
                            className="text-sm"
                            onClick={() => setPersonaValueFilter(value)}
                          >
                            {t(labelKey)}
                          </span>
                        </label>
                      ))}
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("personaReadingStyle")}</label>
                    <div className="space-y-1.5">
                      {(
                        [
                          { value: "academic", labelKey: "personaStyleAcademic" },
                          { value: "practitioner", labelKey: "personaStylePractitioner" },
                          { value: "casual", labelKey: "personaStyleCasual" },
                        ] as const
                      ).map(({ value, labelKey }) => (
                        <label
                          key={value}
                          className="flex items-center gap-2.5 cursor-pointer group"
                        >
                          <div
                            className={cn(
                              "w-4 h-4 rounded-full border-2 flex items-center justify-center transition-colors",
                              personaReadingStyle === value
                                ? "border-primary"
                                : "border-border group-hover:border-primary/50"
                            )}
                            onClick={() => setPersonaReadingStyle(value)}
                          >
                            {personaReadingStyle === value && (
                              <div className="w-2 h-2 rounded-full bg-primary" />
                            )}
                          </div>
                          <span
                            className="text-sm"
                            onClick={() => setPersonaReadingStyle(value)}
                          >
                            {t(labelKey)}
                          </span>
                        </label>
                      ))}
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("personaOutputPrefs")}</label>
                    <div className="space-y-1.5">
                      {OUTPUT_PREF_OPTIONS.map(({ id, labelKey }) => (
                        <label
                          key={id}
                          className="flex items-center gap-2.5 cursor-pointer"
                        >
                          <div
                            className={cn(
                              "w-4 h-4 rounded border-2 flex items-center justify-center transition-colors",
                              personaOutputPrefs.includes(id)
                                ? "border-primary bg-primary"
                                : "border-border hover:border-primary/50"
                            )}
                            onClick={() => toggleOutputPref(id)}
                          >
                            {personaOutputPrefs.includes(id) && (
                              <svg
                                className="w-2.5 h-2.5 text-primary-foreground"
                                fill="none"
                                viewBox="0 0 12 12"
                              >
                                <path
                                  d="M2 6l3 3 5-5"
                                  stroke="currentColor"
                                  strokeWidth="2"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                />
                              </svg>
                            )}
                          </div>
                          <span
                            className="text-sm"
                            onClick={() => toggleOutputPref(id)}
                          >
                            {t(labelKey)}
                          </span>
                        </label>
                      ))}
                    </div>
                  </div>

                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("personaCustomPrompt")}</label>
                    <p className="text-xs text-muted-foreground">
                      {t("personaCustomPromptTip")}
                    </p>
                    <textarea
                      className="w-full min-h-[80px] rounded-md border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring resize-none"
                      defaultValue={settings.persona.customPrompt}
                      placeholder={t("personaCustomPromptPlaceholder")}
                      onChange={(e) => {
                        personaCustomPromptRef.current = e.target.value;
                      }}
                    />
                  </div>
                </>
              )}
            </div>

            <div className="border-t p-4">
              <Button size="lg" className="w-full" type="submit">
                {t("saveChanges")}
              </Button>
            </div>
          </form>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ── Reusable setting group ────────────────────────────────────────────────────

function SettingGroup({
  icon,
  iconBg,
  label,
  children,
}: {
  icon: React.ReactNode;
  iconBg: string;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <div className={cn("size-7 rounded-full p-1.5", iconBg)}>{icon}</div>
        <h4 className="text-sm font-semibold">{label}</h4>
      </div>
      {children}
    </div>
  );
}

