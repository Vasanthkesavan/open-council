export interface AgentMeta {
  key: string;
  label: string;
  emoji: string;
  color: string;
  role: string;
  builtin: boolean;
  sort_order: number;
  voice_gender: string; // "male" | "female"
}

export const COLOR_MAP: Record<
  string,
  { color: string; bgColor: string }
> = {
  blue: { color: "text-blue-400", bgColor: "border-blue-500/30" },
  purple: { color: "text-purple-400", bgColor: "border-purple-500/30" },
  red: { color: "text-red-400", bgColor: "border-red-500/30" },
  teal: { color: "text-teal-400", bgColor: "border-teal-500/30" },
  orange: { color: "text-orange-400", bgColor: "border-orange-500/30" },
  amber: { color: "text-amber-400", bgColor: "border-amber-500/40" },
  green: { color: "text-green-400", bgColor: "border-green-500/30" },
  pink: { color: "text-pink-400", bgColor: "border-pink-500/30" },
  cyan: { color: "text-cyan-400", bgColor: "border-cyan-500/30" },
  indigo: { color: "text-indigo-400", bgColor: "border-indigo-500/30" },
};

const DEFAULT_STYLE = { color: "text-muted-foreground", bgColor: "border-border" };

export function resolveAgentStyle(colorName: string) {
  return COLOR_MAP[colorName] ?? DEFAULT_STYLE;
}

export function resolveAgentConfig(
  agent: string,
  registry: AgentMeta[]
): { emoji: string; label: string; color: string; bgColor: string } {
  const meta = registry.find((a) => a.key === agent);
  if (meta) {
    const style = resolveAgentStyle(meta.color);
    return { emoji: meta.emoji, label: meta.label, ...style };
  }
  return { emoji: "?", label: agent, ...DEFAULT_STYLE };
}
