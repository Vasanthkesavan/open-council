import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Plus,
  ChevronsLeft,
  Settings,
  Trash2,
  Moon,
  Sun,
  Scale,
  Circle,
  Disc,
  Diamond,
  Check,
  Star,
  User,
  MessagesSquare,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

interface Conversation {
  id: string;
  title: string;
  type: string;
  created_at: string;
  updated_at: string;
}

interface Decision {
  id: string;
  conversation_id: string;
  title: string;
  status: string;
  created_at: string;
  updated_at: string;
}

interface SidebarProps {
  currentConversationId: string | null;
  onSelectConversation: (id: string) => void;
  onSelectDecision: (conversationId: string, decisionId: string) => void;
  onNewChat: () => void;
  onNewDecision: () => void;
  onOpenSettings: () => void;
  onOpenProfile: () => void;
  onOpenCommittee: () => void;
  onToggleTheme: () => void;
  onClose: () => void;
  theme: "light" | "dark";
  refreshKey: number;
}

const STATUS_ICONS: Record<
  string,
  { Icon: LucideIcon; color: string; label: string }
> = {
  exploring: { Icon: Circle, color: "text-muted-foreground", label: "Exploring" },
  analyzing: { Icon: Disc, color: "text-blue-500", label: "Analyzing" },
  debating: { Icon: MessagesSquare, color: "text-cyan-500", label: "Debating" },
  recommended: { Icon: Diamond, color: "text-amber-500", label: "Recommended" },
  decided: { Icon: Check, color: "text-green-500", label: "Decided" },
  reviewed: { Icon: Star, color: "text-purple-500", label: "Reviewed" },
};

export default function Sidebar({
  currentConversationId,
  onSelectConversation,
  onSelectDecision,
  onNewChat,
  onNewDecision,
  onOpenSettings,
  onOpenProfile,
  onOpenCommittee,
  onToggleTheme,
  onClose,
  theme,
  refreshKey,
}: SidebarProps) {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [decisions, setDecisions] = useState<Decision[]>([]);

  useEffect(() => {
    loadConversations();
    loadDecisions();
  }, [refreshKey]);

  async function loadConversations() {
    try {
      const convs = await invoke<Conversation[]>("get_conversations");
      setConversations(convs);
    } catch (err) {
      console.error("Failed to load conversations:", err);
    }
  }

  async function loadDecisions() {
    try {
      const decs = await invoke<Decision[]>("get_decisions");
      setDecisions(decs);
    } catch (err) {
      console.error("Failed to load decisions:", err);
    }
  }

  async function handleDelete(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    try {
      await invoke("delete_conversation", { conversationId: id });
      if (currentConversationId === id) {
        onNewChat();
      }
      loadConversations();
      loadDecisions();
    } catch (err) {
      console.error("Failed to delete conversation:", err);
    }
  }

  async function handleDeleteDecision(e: React.MouseEvent, conversationId: string) {
    e.stopPropagation();
    try {
      await invoke("delete_conversation", { conversationId });
      if (currentConversationId === conversationId) {
        onNewChat();
      }
      loadDecisions();
    } catch (err) {
      console.error("Failed to delete decision:", err);
    }
  }

  function formatDate(dateStr: string) {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    if (days === 0) return "Today";
    if (days === 1) return "Yesterday";
    if (days < 7) return `${days}d ago`;
    return date.toLocaleDateString();
  }

  return (
    <div className="w-[280px] bg-sidebar border-r border-sidebar-border flex flex-col h-full shrink-0">
      <div className="p-3 flex items-center justify-between border-b border-sidebar-border">
        <h1 className="text-sm font-semibold text-sidebar-foreground">Open Council</h1>
        <Button
          variant="ghost"
          size="icon"
          onClick={onClose}
          className="h-8 w-8 text-sidebar-foreground/50 hover:text-sidebar-foreground hover:bg-sidebar-accent"
        >
          <ChevronsLeft className="h-4 w-4" />
        </Button>
      </div>

      <ScrollArea className="flex-1">
        {/* Decisions Section */}
        <div className="px-3 pt-3 pb-1">
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs font-semibold uppercase tracking-wider text-sidebar-foreground/40">
              Decisions
            </span>
          </div>
          <Button
            variant="secondary"
            onClick={onNewDecision}
            className="w-full justify-start gap-2 mb-1"
            size="sm"
          >
            <Scale className="h-3.5 w-3.5" />
            New Decision
          </Button>
        </div>

        <div className="px-2">
          {decisions.map((dec) => {
            const statusInfo = STATUS_ICONS[dec.status] || STATUS_ICONS.exploring;
            return (
              <div
                key={dec.id}
                onClick={() => onSelectDecision(dec.conversation_id, dec.id)}
                className={cn(
                  "group px-3 py-2 rounded-lg cursor-pointer mb-0.5 flex items-center justify-between transition-colors",
                  currentConversationId === dec.conversation_id
                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                    : "text-sidebar-foreground/60 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
                )}
              >
                <div className="min-w-0 flex-1 flex items-center gap-2">
                  <span title={statusInfo.label} aria-label={statusInfo.label}>
                    <statusInfo.Icon
                      className={cn("h-3.5 w-3.5 shrink-0", statusInfo.color)}
                    />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm truncate">{dec.title}</div>
                    <div className="text-xs text-sidebar-foreground/30 mt-0.5">
                      {formatDate(dec.updated_at)}
                    </div>
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={(e) => handleDeleteDecision(e, dec.conversation_id)}
                  className="opacity-0 group-hover:opacity-100 h-7 w-7 text-sidebar-foreground/40 hover:text-sidebar-foreground hover:bg-sidebar-accent shrink-0 ml-2"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            );
          })}
        </div>

        <Separator className="bg-sidebar-border mx-3 my-2" />

        {/* Conversations Section */}
        <div className="px-3 pb-1">
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs font-semibold uppercase tracking-wider text-sidebar-foreground/40">
              Conversations
            </span>
          </div>
          <Button
            variant="secondary"
            onClick={onNewChat}
            className="w-full justify-start gap-2 mb-1"
            size="sm"
          >
            <Plus className="h-3.5 w-3.5" />
            New Chat
          </Button>
        </div>

        <div className="px-2">
          {conversations.map((conv) => (
            <div
              key={conv.id}
              onClick={() => onSelectConversation(conv.id)}
              className={cn(
                "group px-3 py-2.5 rounded-lg cursor-pointer mb-0.5 flex items-center justify-between transition-colors",
                currentConversationId === conv.id
                  ? "bg-sidebar-accent text-sidebar-accent-foreground"
                  : "text-sidebar-foreground/60 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
              )}
            >
              <div className="min-w-0 flex-1">
                <div className="text-sm truncate">{conv.title}</div>
                <div className="text-xs text-sidebar-foreground/30 mt-0.5">
                  {formatDate(conv.updated_at)}
                </div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={(e) => handleDelete(e, conv.id)}
                className="opacity-0 group-hover:opacity-100 h-7 w-7 text-sidebar-foreground/40 hover:text-sidebar-foreground hover:bg-sidebar-accent shrink-0 ml-2"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          ))}
        </div>
      </ScrollArea>

      <Separator className="bg-sidebar-border" />

      <div className="p-3 space-y-1">
        <Button
          variant="ghost"
          onClick={onToggleTheme}
          aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
          className="w-full justify-start gap-2 text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent"
        >
          {theme === "dark" ? (
            <Sun className="h-4 w-4" />
          ) : (
            <Moon className="h-4 w-4" />
          )}
          {theme === "dark" ? "Light Mode" : "Dark Mode"}
        </Button>
        <Button
          variant="ghost"
          onClick={onOpenProfile}
          className="w-full justify-start gap-2 text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent"
        >
          <User className="h-4 w-4" />
          Profile
        </Button>
        <Button
          variant="ghost"
          onClick={onOpenCommittee}
          className="w-full justify-start gap-2 text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent"
        >
          <MessagesSquare className="h-4 w-4" />
          Committee
        </Button>
        <Button
          variant="ghost"
          onClick={onOpenSettings}
          className="w-full justify-start gap-2 text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent"
        >
          <Settings className="h-4 w-4" />
          Settings
        </Button>
      </div>
    </div>
  );
}
