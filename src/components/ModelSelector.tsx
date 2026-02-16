import { useState, useRef, useEffect } from "react";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

export interface ModelInfo {
  id: string;
  name: string;
  input: string;
  output: string;
  context: string;
  tier: "premium" | "recommended" | "value" | "budget" | "free";
}

export const MODELS: ModelInfo[] = [
  // Premium
  { id: "anthropic/claude-opus-4-6", name: "Claude Opus 4.6", input: "$5", output: "$25", context: "1M", tier: "premium" },
  { id: "openai/gpt-5.2-codex", name: "GPT-5.2 Codex", input: "$1.75", output: "$14", context: "400K", tier: "premium" },
  { id: "writer/palmyra-x5", name: "Palmyra X5", input: "$0.60", output: "$6", context: "1M", tier: "premium" },
  { id: "qwen/qwen3-max-thinking", name: "Qwen3 Max Thinking", input: "$1.20", output: "$6", context: "262K", tier: "premium" },

  // Recommended
  { id: "anthropic/claude-sonnet-4-5", name: "Claude Sonnet 4.5", input: "$3", output: "$15", context: "200K", tier: "recommended" },
  { id: "google/gemini-3-flash-preview", name: "Gemini 3 Flash Preview", input: "$0.50", output: "$3", context: "1M", tier: "recommended" },
  { id: "moonshotai/kimi-k2.5", name: "Kimi K2.5", input: "$0.23", output: "$3", context: "262K", tier: "recommended" },
  { id: "deepseek/deepseek-chat-v3-0324", name: "DeepSeek Chat v3", input: "$0.14", output: "$0.28", context: "128K", tier: "recommended" },

  // Value
  { id: "qwen/qwen3.5-plus-2026-02-15", name: "Qwen3.5 Plus", input: "$0.40", output: "$2.40", context: "1M", tier: "value" },
  { id: "qwen/qwen3.5-397b-a17b", name: "Qwen3.5 397B A17B", input: "$0.60", output: "$3.60", context: "262K", tier: "value" },
  { id: "minimax/minimax-m2.5", name: "MiniMax M2.5", input: "$0.30", output: "$1.20", context: "196K", tier: "value" },
  { id: "z-ai/glm-5", name: "GLM 5", input: "$0.30", output: "$2.55", context: "204K", tier: "value" },
  { id: "bytedance-seed/seed-1.6", name: "Seed 1.6", input: "$0.25", output: "$2", context: "262K", tier: "value" },
  { id: "z-ai/glm-4.7", name: "GLM 4.7", input: "$0.40", output: "$1.50", context: "202K", tier: "value" },
  { id: "minimax/minimax-m2.1", name: "MiniMax M2.1", input: "$0.27", output: "$0.95", context: "196K", tier: "value" },

  // Budget
  { id: "stepfun/step-3.5-flash", name: "Step 3.5 Flash", input: "$0.10", output: "$0.30", context: "256K", tier: "budget" },
  { id: "mistralai/mistral-small-creative", name: "Mistral Small Creative", input: "$0.10", output: "$0.30", context: "32K", tier: "budget" },
  { id: "xiaomi/mimo-v2-flash", name: "MiMo V2 Flash", input: "$0.09", output: "$0.29", context: "262K", tier: "budget" },
  { id: "bytedance-seed/seed-1.6-flash", name: "Seed 1.6 Flash", input: "$0.075", output: "$0.30", context: "262K", tier: "budget" },
  { id: "qwen/qwen3-coder-next", name: "Qwen3 Coder Next", input: "$0.07", output: "$0.30", context: "262K", tier: "budget" },
  { id: "z-ai/glm-4.7-flash", name: "GLM 4.7 Flash", input: "$0.06", output: "$0.40", context: "202K", tier: "budget" },
  { id: "allenai/olmo-3.1-32b-instruct", name: "OLMo 3.1 32B", input: "$0.20", output: "$0.60", context: "65K", tier: "budget" },

  // Free
  { id: "openrouter/aurora-alpha", name: "Aurora Alpha", input: "Free", output: "Free", context: "128K", tier: "free" },
  { id: "stepfun/step-3.5-flash:free", name: "Step 3.5 Flash (free)", input: "Free", output: "Free", context: "256K", tier: "free" },
  { id: "arcee-ai/trinity-large-preview:free", name: "Trinity Large Preview (free)", input: "Free", output: "Free", context: "131K", tier: "free" },
  { id: "upstage/solar-pro-3:free", name: "Solar Pro 3 (free)", input: "Free", output: "Free", context: "128K", tier: "free" },
  { id: "nvidia/nemotron-3-nano-30b-a3b:free", name: "Nemotron 3 Nano 30B (free)", input: "Free", output: "Free", context: "256K", tier: "free" },
  { id: "liquid/lfm2.5-1.2b-thinking:free", name: "LFM2.5 Thinking (free)", input: "Free", output: "Free", context: "32K", tier: "free" },
];

const TIER_LABELS: Record<ModelInfo["tier"], string> = {
  premium: "Premium",
  recommended: "Recommended",
  value: "Value",
  budget: "Budget",
  free: "Free",
};

const TIER_COLORS: Record<ModelInfo["tier"], string> = {
  premium: "text-amber-500",
  recommended: "text-emerald-500",
  value: "text-blue-400",
  budget: "text-violet-400",
  free: "text-muted-foreground",
};

interface ModelSelectorProps {
  value: string;
  onChange: (modelId: string) => void;
  placeholder?: string;
  compact?: boolean;
}

export default function ModelSelector({
  value,
  onChange,
  placeholder = "Search models...",
  compact = false,
}: ModelSelectorProps) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Resolve display name for current value
  const selectedModel = MODELS.find((m) => m.id === value);
  const displayValue = open ? query : (selectedModel ? selectedModel.name : value);

  // Filter models by query
  const filtered = query.trim()
    ? MODELS.filter((m) => {
        const q = query.toLowerCase();
        return (
          m.id.toLowerCase().includes(q) ||
          m.name.toLowerCase().includes(q) ||
          m.tier.includes(q)
        );
      })
    : MODELS;

  // Group filtered models by tier
  const grouped = new Map<ModelInfo["tier"], ModelInfo[]>();
  for (const m of filtered) {
    const list = grouped.get(m.tier) || [];
    list.push(m);
    grouped.set(m.tier, list);
  }

  // Close on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
        setQuery("");
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  function handleSelect(modelId: string) {
    onChange(modelId);
    setOpen(false);
    setQuery("");
    inputRef.current?.blur();
  }

  function handleFocus() {
    setOpen(true);
    setQuery("");
  }

  function handleInputChange(val: string) {
    setQuery(val);
    if (!open) setOpen(true);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      setOpen(false);
      setQuery("");
      inputRef.current?.blur();
    }
    if (e.key === "Enter" && query.trim()) {
      // If query matches a model exactly, select it; otherwise use as custom ID
      const exact = MODELS.find(
        (m) => m.id === query.trim() || m.name.toLowerCase() === query.trim().toLowerCase()
      );
      handleSelect(exact ? exact.id : query.trim());
    }
  }

  return (
    <div ref={containerRef} className="relative">
      <Input
        ref={inputRef}
        type="text"
        value={displayValue}
        onChange={(e) => handleInputChange(e.target.value)}
        onFocus={handleFocus}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        className={compact ? "h-7 text-xs font-mono" : "font-mono text-sm"}
      />

      {open && (
        <div className="absolute z-50 mt-1 w-full rounded-md border border-border bg-popover shadow-lg">
          <ScrollArea className="max-h-[320px]">
            <div className="p-1">
              {filtered.length === 0 ? (
                <div className="px-3 py-6 text-center">
                  <p className="text-xs text-muted-foreground">No matching models</p>
                  <p className="text-[11px] text-muted-foreground mt-1">
                    Press Enter to use "<span className="font-mono">{query}</span>" as a custom model ID
                  </p>
                </div>
              ) : (
                Array.from(grouped.entries()).map(([tier, models]) => (
                  <div key={tier}>
                    <div className={`px-2 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wider ${TIER_COLORS[tier]}`}>
                      {TIER_LABELS[tier]}
                    </div>
                    {models.map((m) => (
                      <button
                        key={m.id}
                        type="button"
                        onClick={() => handleSelect(m.id)}
                        className={`w-full text-left px-2 py-1.5 rounded-sm text-xs transition-colors hover:bg-accent hover:text-accent-foreground ${
                          value === m.id ? "bg-accent/50" : ""
                        }`}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="font-medium truncate">{m.name}</span>
                          <span className="text-[10px] text-muted-foreground shrink-0">{m.context}</span>
                        </div>
                        <div className="flex items-center justify-between gap-2 mt-0.5">
                          <span className="font-mono text-[10px] text-muted-foreground truncate">{m.id}</span>
                          <span className="text-[10px] text-muted-foreground shrink-0">
                            {m.input === "Free" ? "Free" : `${m.input} / ${m.output}`}
                          </span>
                        </div>
                      </button>
                    ))}
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
          <div className="border-t border-border px-3 py-1.5">
            <p className="text-[10px] text-muted-foreground">
              Type any model ID from openrouter.ai/models
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
