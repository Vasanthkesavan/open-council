import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Settings as SettingsIcon, FolderOpen } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import ModelSelector from "@/components/ModelSelector";

interface SettingsProps {
  onClose: () => void;
  onSaved: () => void;
  mustSetKey: boolean;
}

interface SettingsResponse {
  api_key_set: boolean;
  api_key_preview: string;
  model: string;
}

export default function Settings({ onClose, onSaved, mustSetKey }: SettingsProps) {
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("anthropic/claude-sonnet-4-5");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [currentPreview, setCurrentPreview] = useState("");
  const [hasExistingKey, setHasExistingKey] = useState(false);

  useEffect(() => {
    loadSettings();
  }, []);

  async function loadSettings() {
    try {
      const settings = await invoke<SettingsResponse>("get_settings");
      setModel(settings.model);
      setCurrentPreview(settings.api_key_preview);
      setHasExistingKey(settings.api_key_set);
    } catch (err) {
      console.error("Failed to load settings:", err);
    }
  }

  async function handleSave() {
    if (mustSetKey && !apiKey.trim() && !hasExistingKey) {
      setError("Please enter your OpenRouter API key to get started.");
      return;
    }

    setSaving(true);
    setError(null);

    try {
      await invoke("save_settings", {
        apiKey: apiKey.trim(),
        model: model.trim(),
      });
      onSaved();
    } catch (err) {
      setError(typeof err === "string" ? err : "Failed to save settings.");
    } finally {
      setSaving(false);
    }
  }

  async function handleOpenFolder() {
    try {
      const path = await invoke<string>("open_profile_folder");
      await revealItemInDir(path);
    } catch (err) {
      console.error("Failed to open folder:", err);
    }
  }

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open && !mustSetKey) {
          onClose();
        }
      }}
    >
      <DialogContent
        className={`sm:max-w-md ${mustSetKey ? "[&>button.absolute]:hidden" : ""}`}
        onInteractOutside={(e) => {
          if (mustSetKey) e.preventDefault();
        }}
        onEscapeKeyDown={(e) => {
          if (mustSetKey) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <SettingsIcon className="h-5 w-5" />
            {mustSetKey ? "Welcome! Set up OpenRouter" : "Settings"}
          </DialogTitle>
          {mustSetKey && (
            <DialogDescription>
              Open Council uses OpenRouter to access AI models. Enter your
              API key to get started.
            </DialogDescription>
          )}
        </DialogHeader>

        <Separator />

        <div className="space-y-5">
          {/* API Key */}
          <div>
            <label className="text-sm font-medium text-muted-foreground block mb-1.5">
              OpenRouter API Key
            </label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={currentPreview || "sk-or-v1-..."}
            />
            {hasExistingKey && !apiKey && (
              <p className="text-xs text-muted-foreground mt-1">
                Current key: {currentPreview}. Leave blank to keep it.
              </p>
            )}
            <p className="text-xs text-muted-foreground mt-1">
              Don't have one?{" "}
              <span className="text-foreground font-medium">
                Get a free key at openrouter.ai/keys
              </span>
            </p>
          </div>

          {/* Model */}
          <div>
            <label className="text-sm font-medium text-muted-foreground block mb-1.5">
              Default Model
            </label>
            <ModelSelector value={model} onChange={setModel} />
          </div>

          {/* Profile Files */}
          <div>
            <label className="text-sm font-medium text-muted-foreground block mb-1.5">
              Profile Files
            </label>
            <Button
              variant="outline"
              className="w-full justify-start"
              onClick={handleOpenFolder}
            >
              <FolderOpen className="h-4 w-4" />
              Open Profile Folder
            </Button>
          </div>

          {error && (
            <div className="px-3 py-2.5 rounded-lg bg-destructive/15 border border-destructive/30 text-destructive text-sm">
              {error}
            </div>
          )}
        </div>

        <Separator />

        <DialogFooter>
          {!mustSetKey && (
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
          )}
          <Button onClick={handleSave} disabled={saving}>
            {saving ? "Saving..." : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
