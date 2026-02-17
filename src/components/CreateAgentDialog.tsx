import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AgentMeta } from "@/lib/agentColors";
import { Loader2 } from "lucide-react";

const EMOJI_OPTIONS = [
  "\u{1f4b0}", // 💰
  "\u{1f3af}", // 🎯
  "\u{2696}\u{fe0f}", // ⚖️
  "\u{1f9e0}", // 🧠
  "\u{1f4ca}", // 📊
  "\u{1f30d}", // 🌍
  "\u{2764}\u{fe0f}", // ❤️
  "\u{1f680}", // 🚀
  "\u{1f3d7}\u{fe0f}", // 🏗️
  "\u{1f50d}", // 🔍
  "\u{1f4d6}", // 📖
  "\u{1f3c6}", // 🏆
  "\u{1f9ea}", // 🧪
  "\u{1f4a1}", // 💡
  "\u{1f6e1}\u{fe0f}", // 🛡️
];

interface CreateAgentDialogProps {
  onCreated: (agent: AgentMeta) => void;
  onClose: () => void;
}

export default function CreateAgentDialog({
  onCreated,
  onClose,
}: CreateAgentDialogProps) {
  const [name, setName] = useState("");
  const [emoji, setEmoji] = useState(EMOJI_OPTIONS[0]);
  const [voiceGender, setVoiceGender] = useState<"male" | "female">("male");
  const [description, setDescription] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleCreate() {
    if (!name.trim() || !description.trim()) return;

    setLoading(true);
    setError(null);
    try {
      const agent = await invoke<AgentMeta>("create_custom_agent", {
        label: name.trim(),
        emoji,
        description: description.trim(),
        voiceGender,
      });
      onCreated(agent);
    } catch (err) {
      setError(typeof err === "string" ? err : "Failed to create agent");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !loading && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add Committee Member</DialogTitle>
          <DialogDescription>
            Give a brief description and we&apos;ll generate a detailed prompt
            for your new committee member.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div>
            <label className="text-xs font-medium text-muted-foreground block mb-1.5">
              Name
            </label>
            <Input
              placeholder="e.g. Economist, Ethicist, Risk Analyst"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={loading}
            />
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground block mb-1.5">
              Emoji
            </label>
            <div className="flex flex-wrap gap-1">
              {EMOJI_OPTIONS.map((e) => (
                <button
                  key={e}
                  type="button"
                  onClick={() => setEmoji(e)}
                  disabled={loading}
                  className={`w-8 h-8 rounded flex items-center justify-center text-lg transition-colors ${
                    emoji === e
                      ? "bg-accent ring-2 ring-primary"
                      : "hover:bg-muted"
                  }`}
                >
                  {e}
                </button>
              ))}
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground block mb-1.5">
              Voice
            </label>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setVoiceGender("male")}
                disabled={loading}
                className={`flex-1 py-2 px-3 rounded-md text-sm transition-colors border ${
                  voiceGender === "male"
                    ? "bg-accent border-primary ring-1 ring-primary"
                    : "border-border hover:bg-muted"
                }`}
              >
                Male
              </button>
              <button
                type="button"
                onClick={() => setVoiceGender("female")}
                disabled={loading}
                className={`flex-1 py-2 px-3 rounded-md text-sm transition-colors border ${
                  voiceGender === "female"
                    ? "bg-accent border-primary ring-1 ring-primary"
                    : "border-border hover:bg-muted"
                }`}
              >
                Female
              </button>
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-muted-foreground block mb-1.5">
              What perspective should this member bring?
            </label>
            <Textarea
              placeholder="e.g. Focuses on financial impact, ROI calculations, and economic tradeoffs. Evaluates decisions through the lens of cost-benefit analysis and long-term financial sustainability."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              disabled={loading}
              rows={4}
            />
          </div>

          {error && (
            <p className="text-xs text-destructive">{error}</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={loading}>
            Cancel
          </Button>
          <Button
            onClick={handleCreate}
            disabled={loading || !name.trim() || !description.trim()}
          >
            {loading ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                Generating prompt...
              </>
            ) : (
              "Create Member"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
