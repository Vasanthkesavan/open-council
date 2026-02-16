import { useState, useEffect, useLayoutEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Menu } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import Sidebar from "./components/Sidebar";
import ChatView from "./components/ChatView";
import DecisionView from "./components/DecisionView";
import ProfileView from "./components/ProfileView";
import CommitteeView from "./components/CommitteeView";
import Settings from "./components/Settings";
import "./App.css";

interface SettingsResponse {
  api_key_set: boolean;
  api_key_preview: string;
  model: string;
}

interface CreateDecisionResponse {
  conversation_id: string;
  decision_id: string;
}

type Theme = "light" | "dark";
const THEME_STORAGE_KEY = "decision-copilot-theme";

type ViewMode = "chat" | "decision" | "profile" | "committee";

function getInitialTheme(): Theme {
  if (typeof window === "undefined") {
    return "dark";
  }

  const savedTheme = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (savedTheme === "light" || savedTheme === "dark") {
    return savedTheme;
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function App() {
  const [currentConversationId, setCurrentConversationId] = useState<string | null>(null);
  const [currentDecisionId, setCurrentDecisionId] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("chat");
  const [showSettings, setShowSettings] = useState(false);
  const [showNewDecisionInput, setShowNewDecisionInput] = useState(false);
  const [newDecisionTitle, setNewDecisionTitle] = useState("");
  const [apiKeySet, setApiKeySet] = useState<boolean | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const [activeModel, setActiveModel] = useState("");
  const [theme, setTheme] = useState<Theme>(() => getInitialTheme());

  useEffect(() => {
    checkApiKey();
  }, []);

  useLayoutEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.documentElement.style.colorScheme = theme;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  async function checkApiKey() {
    try {
      const settings = await invoke<SettingsResponse>("get_settings");
      const configured = settings.api_key_set;
      setApiKeySet(configured);
      setActiveModel(settings.model);
      if (!configured) {
        setShowSettings(true);
      }
    } catch {
      setApiKeySet(false);
      setShowSettings(true);
    }
  }

  function handleNewChat() {
    setCurrentConversationId(null);
    setCurrentDecisionId(null);
    setViewMode("chat");
  }

  function handleSelectConversation(id: string) {
    setCurrentConversationId(id);
    setCurrentDecisionId(null);
    setViewMode("chat");
  }

  function handleSelectDecision(conversationId: string, decisionId: string) {
    setCurrentConversationId(conversationId);
    setCurrentDecisionId(decisionId);
    setViewMode("decision");
  }

  function handleNewDecision() {
    setShowNewDecisionInput(true);
    setNewDecisionTitle("");
  }

  function handleOpenProfile() {
    setViewMode("profile");
  }

  function handleOpenCommittee() {
    setViewMode("committee");
  }

  async function handleCreateDecision() {
    const title = newDecisionTitle.trim();
    if (!title) return;

    try {
      const result = await invoke<CreateDecisionResponse>("create_decision", { title });
      setCurrentConversationId(result.conversation_id);
      setCurrentDecisionId(result.decision_id);
      setViewMode("decision");
      setShowNewDecisionInput(false);
      setNewDecisionTitle("");
      setRefreshKey((k) => k + 1);
    } catch (err) {
      console.error("Failed to create decision:", err);
    }
  }

  function handleSettingsSaved() {
    setApiKeySet(true);
    setShowSettings(false);
    checkApiKey();
  }

  function handleConversationCreated(id: string) {
    setCurrentConversationId(id);
    setRefreshKey((k) => k + 1);
  }

  function handleMessageSent() {
    setRefreshKey((k) => k + 1);
  }

  function handleToggleTheme() {
    setTheme((current) => (current === "dark" ? "light" : "dark"));
  }

  if (apiKeySet === null) {
    return (
      <div className="h-screen bg-background text-foreground flex items-center justify-center">
        <div className="text-muted-foreground">Loading...</div>
      </div>
    );
  }

  return (
    <div className="h-screen bg-background text-foreground flex overflow-hidden transition-colors">
      {sidebarOpen && (
        <Sidebar
          currentConversationId={currentConversationId}
          onSelectConversation={handleSelectConversation}
          onSelectDecision={handleSelectDecision}
          onNewChat={handleNewChat}
          onNewDecision={handleNewDecision}
          onOpenSettings={() => setShowSettings(true)}
          onOpenProfile={handleOpenProfile}
          onOpenCommittee={handleOpenCommittee}
          onToggleTheme={handleToggleTheme}
          onClose={() => setSidebarOpen(false)}
          theme={theme}
          refreshKey={refreshKey}
        />
      )}
      <div className="flex-1 flex flex-col min-w-0">
        {!sidebarOpen && (
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setSidebarOpen(true)}
            className="absolute top-3 left-3 z-10"
          >
            <Menu className="h-5 w-5" />
          </Button>
        )}
        {viewMode === "profile" ? (
          <ProfileView onNavigateToChat={handleNewChat} />
        ) : viewMode === "committee" ? (
          <CommitteeView onNavigateToChat={handleNewChat} />
        ) : viewMode === "decision" && currentConversationId && currentDecisionId ? (
          <DecisionView
            conversationId={currentConversationId}
            decisionId={currentDecisionId}
            onMessageSent={handleMessageSent}
            activeModel={activeModel}
          />
        ) : (
          <ChatView
            conversationId={currentConversationId}
            onConversationCreated={handleConversationCreated}
            onMessageSent={handleMessageSent}
            activeModel={activeModel}
          />
        )}
      </div>
      {showSettings && (
        <Settings
          onClose={() => {
            if (apiKeySet) setShowSettings(false);
          }}
          onSaved={handleSettingsSaved}
          mustSetKey={!apiKeySet}
        />
      )}

      {/* New Decision Modal */}
      {showNewDecisionInput && (
        <Dialog open onOpenChange={(open) => !open && setShowNewDecisionInput(false)}>
          <DialogContent className="sm:max-w-md">
            <DialogHeader>
              <DialogTitle>New Decision</DialogTitle>
              <DialogDescription>
                What decision are you working through?
              </DialogDescription>
            </DialogHeader>
            <Input
              value={newDecisionTitle}
              onChange={(e) => setNewDecisionTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleCreateDecision();
              }}
              placeholder="e.g., Should I leave my job?"
              autoFocus
            />
            <DialogFooter>
              <Button
                variant="ghost"
                onClick={() => setShowNewDecisionInput(false)}
              >
                Cancel
              </Button>
              <Button
                onClick={handleCreateDecision}
                disabled={!newDecisionTitle.trim()}
              >
                Start
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}

export default App;
