import type { AgentMeta } from "@/lib/agentColors";

interface AudioSegment {
  index: number;
  agent: string;
  round: number;
  exchange: number;
  text: string;
  audio_file: string;
  duration_ms: number;
  start_ms: number;
}

interface SpeakerTimelineProps {
  segments: AudioSegment[];
  currentSegmentIndex: number;
  registry: AgentMeta[];
  onSegmentClick: (index: number) => void;
}

export default function SpeakerTimeline({
  segments,
  currentSegmentIndex,
  registry,
  onSegmentClick,
}: SpeakerTimelineProps) {
  const totalDuration = segments.reduce((sum, s) => sum + s.duration_ms, 0);
  if (totalDuration === 0) return null;

  return (
    <div className="flex gap-px h-2 rounded-full overflow-hidden bg-muted">
      {segments.map((segment) => {
        const meta = registry.find((a) => a.key === segment.agent);
        const widthPercent = (segment.duration_ms / totalDuration) * 100;
        const isActive = segment.index === currentSegmentIndex;

        return (
          <button
            key={segment.index}
            type="button"
            onClick={() => onSegmentClick(segment.index)}
            className={`h-full transition-opacity ${
              isActive ? "opacity-100" : "opacity-40 hover:opacity-70"
            }`}
            style={{
              width: `${widthPercent}%`,
              minWidth: "4px",
              backgroundColor: `var(--color-${meta?.color || "blue"}-500, #3b82f6)`,
            }}
            title={`${meta?.emoji || ""} ${meta?.label || segment.agent} (Round ${segment.round})`}
          />
        );
      })}
    </div>
  );
}
