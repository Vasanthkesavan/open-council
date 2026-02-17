import type { AgentMeta } from "@/lib/agentColors";
import { resolveAgentConfig } from "@/lib/agentColors";

interface AudioGenerationProgressProps {
  completed: number;
  total: number;
  currentAgent: string;
  registry: AgentMeta[];
}

export default function AudioGenerationProgress({
  completed,
  total,
  currentAgent,
  registry,
}: AudioGenerationProgressProps) {
  const percent = total > 0 ? Math.round((completed / total) * 100) : 0;
  const agentConfig = currentAgent
    ? resolveAgentConfig(currentAgent, registry)
    : null;

  return (
    <div className="px-4 py-3 rounded-lg bg-muted/50 border border-border">
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm text-muted-foreground">
          Generating voices...
        </span>
        <span className="text-xs text-muted-foreground">
          {completed}/{total}
        </span>
      </div>
      <div className="w-full h-1.5 bg-muted rounded-full overflow-hidden">
        <div
          className="h-full bg-primary rounded-full transition-all duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
      {agentConfig && currentAgent && (
        <div className="mt-1.5 flex items-center gap-1.5">
          <span className="text-xs">{agentConfig.emoji}</span>
          <span className="text-xs text-muted-foreground">
            {agentConfig.label}
          </span>
        </div>
      )}
    </div>
  );
}
