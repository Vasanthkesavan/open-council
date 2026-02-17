import { resolveAgentStyle } from "@/lib/agentColors";

interface AudioWaveformProps {
  isActive: boolean;
  color: string;
  size?: "sm" | "md";
}

export default function AudioWaveform({
  isActive,
  color,
  size = "sm",
}: AudioWaveformProps) {
  const style = resolveAgentStyle(color);
  const barCount = size === "md" ? 5 : 4;
  const barHeight = size === "md" ? "h-4" : "h-3";
  const barWidth = size === "md" ? "w-1" : "w-0.5";

  return (
    <div className="flex items-center gap-0.5">
      {Array.from({ length: barCount }).map((_, i) => (
        <div
          key={i}
          className={`${barWidth} rounded-full transition-all ${
            isActive ? style.color.replace("text-", "bg-") : "bg-muted-foreground/30"
          } ${barHeight}`}
          style={
            isActive
              ? {
                  animation: `waveform 0.8s ease-in-out ${i * 0.1}s infinite alternate`,
                }
              : { transform: "scaleY(0.3)" }
          }
        />
      ))}
      <style>{`
        @keyframes waveform {
          0% { transform: scaleY(0.3); }
          100% { transform: scaleY(1); }
        }
      `}</style>
    </div>
  );
}
