import { useState, useEffect, useRef, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  X,
  Gauge,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import type { AgentMeta } from "@/lib/agentColors";
import { resolveAgentConfig } from "@/lib/agentColors";
import AudioWaveform from "./AudioWaveform";
import SpeakerTimeline from "./SpeakerTimeline";

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

interface AudioManifest {
  decision_id: string;
  segments: AudioSegment[];
  total_duration_ms: number;
}

interface AudioPlayerProps {
  manifest: AudioManifest;
  audioDir: string;
  registry: AgentMeta[];
  onClose: () => void;
}

const SPEED_OPTIONS = [1, 1.25, 1.5, 2] as const;
const INTER_SPEAKER_GAP = 500; // ms between speakers
const INTER_ROUND_GAP = 1000; // ms between rounds

function formatTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export default function AudioPlayer({
  manifest,
  audioDir,
  registry,
  onClose,
}: AudioPlayerProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [speedIndex, setSpeedIndex] = useState(0);
  const [currentTime, setCurrentTime] = useState(0); // time within current segment
  const [segmentDuration, setSegmentDuration] = useState(0);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const gapTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const segments = manifest.segments;
  const currentSegment = segments[currentIndex];
  const agentConfig = currentSegment
    ? resolveAgentConfig(currentSegment.agent, registry)
    : null;

  // Global time position
  const globalTime =
    (currentSegment?.start_ms || 0) + currentTime * 1000;

  // Build audio URL for a segment
  const getAudioUrl = useCallback(
    (segment: AudioSegment) => {
      const filePath = `${audioDir}/${segment.audio_file}`.replace(/\\/g, "/");
      return convertFileSrc(filePath);
    },
    [audioDir]
  );

  // Load and play a segment
  const loadSegment = useCallback(
    (index: number, autoplay: boolean) => {
      if (index < 0 || index >= segments.length) return;

      if (gapTimeoutRef.current) {
        clearTimeout(gapTimeoutRef.current);
        gapTimeoutRef.current = null;
      }

      setCurrentIndex(index);
      setCurrentTime(0);
      setSegmentDuration(0);

      const audio = audioRef.current;
      if (!audio) return;

      const url = getAudioUrl(segments[index]);
      audio.src = url;
      audio.playbackRate = SPEED_OPTIONS[speedIndex];
      audio.load();

      if (autoplay) {
        audio.play().catch(console.error);
        setIsPlaying(true);
      }
    },
    [segments, getAudioUrl, speedIndex]
  );

  // Initialize audio element
  useEffect(() => {
    const audio = new Audio();
    audioRef.current = audio;

    audio.addEventListener("timeupdate", () => {
      setCurrentTime(audio.currentTime);
    });

    audio.addEventListener("loadedmetadata", () => {
      setSegmentDuration(audio.duration);
    });

    audio.addEventListener("ended", () => {
      // Advance to next segment with a gap
      const nextIndex = currentIndex + 1;
      if (nextIndex >= segments.length) {
        setIsPlaying(false);
        return;
      }

      const currentRound = segments[currentIndex]?.round;
      const nextRound = segments[nextIndex]?.round;
      const gap =
        currentRound !== nextRound ? INTER_ROUND_GAP : INTER_SPEAKER_GAP;

      gapTimeoutRef.current = setTimeout(() => {
        loadSegment(nextIndex, true);
      }, gap);
    });

    // Load first segment
    if (segments.length > 0) {
      const url = getAudioUrl(segments[0]);
      audio.src = url;
      audio.load();
    }

    return () => {
      audio.pause();
      audio.src = "";
      if (gapTimeoutRef.current) {
        clearTimeout(gapTimeoutRef.current);
      }
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Update the ended handler when currentIndex changes
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const handleEnded = () => {
      const nextIndex = currentIndex + 1;
      if (nextIndex >= segments.length) {
        setIsPlaying(false);
        return;
      }

      const currentRound = segments[currentIndex]?.round;
      const nextRound = segments[nextIndex]?.round;
      const gap =
        currentRound !== nextRound ? INTER_ROUND_GAP : INTER_SPEAKER_GAP;

      gapTimeoutRef.current = setTimeout(() => {
        loadSegment(nextIndex, true);
      }, gap);
    };

    audio.addEventListener("ended", handleEnded);
    return () => audio.removeEventListener("ended", handleEnded);
  }, [currentIndex, segments, loadSegment]);

  function togglePlayPause() {
    const audio = audioRef.current;
    if (!audio) return;

    if (isPlaying) {
      audio.pause();
      setIsPlaying(false);
    } else {
      audio.play().catch(console.error);
      setIsPlaying(true);
    }
  }

  function handlePrev() {
    if (currentTime > 2) {
      // If more than 2 seconds in, restart current segment
      const audio = audioRef.current;
      if (audio) {
        audio.currentTime = 0;
        setCurrentTime(0);
      }
    } else {
      loadSegment(Math.max(0, currentIndex - 1), isPlaying);
    }
  }

  function handleNext() {
    loadSegment(Math.min(segments.length - 1, currentIndex + 1), isPlaying);
  }

  function handleSpeedToggle() {
    const nextIndex = (speedIndex + 1) % SPEED_OPTIONS.length;
    setSpeedIndex(nextIndex);
    const audio = audioRef.current;
    if (audio) {
      audio.playbackRate = SPEED_OPTIONS[nextIndex];
    }
  }

  function handleProgressClick(e: React.MouseEvent<HTMLDivElement>) {
    if (segmentDuration <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percent = x / rect.width;
    const newTime = percent * segmentDuration;
    const audio = audioRef.current;
    if (audio) {
      audio.currentTime = newTime;
      setCurrentTime(newTime);
    }
  }

  function handleSegmentClick(index: number) {
    loadSegment(index, isPlaying);
  }

  if (!currentSegment || !agentConfig) {
    return null;
  }

  const progressPercent =
    segmentDuration > 0 ? (currentTime / segmentDuration) * 100 : 0;

  return (
    <div className="border-t border-border bg-background">
      {/* Speaker row */}
      <div className="px-4 pt-3 pb-2 flex items-center gap-3">
        {segments
          .filter(
            (s, i, arr) => arr.findIndex((a) => a.agent === s.agent) === i
          )
          .map((s) => {
            const config = resolveAgentConfig(s.agent, registry);
            const isActive = s.agent === currentSegment.agent;
            const meta = registry.find((a) => a.key === s.agent);
            return (
              <div
                key={s.agent}
                className={`flex flex-col items-center gap-0.5 transition-opacity ${
                  isActive ? "opacity-100" : "opacity-40"
                }`}
              >
                <span className="text-lg">{config.emoji}</span>
                {isActive && meta && (
                  <AudioWaveform
                    isActive={isPlaying}
                    color={meta.color}
                    size="sm"
                  />
                )}
              </div>
            );
          })}
        <div className="ml-auto">
          <Button variant="ghost" size="sm" onClick={onClose}>
            <X className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      {/* Current speaker + transcript */}
      <div className="px-4 pb-2">
        <div className="flex items-center gap-2 mb-1">
          <span className={`text-xs font-medium ${agentConfig.color}`}>
            {agentConfig.emoji} {agentConfig.label}
          </span>
          {isPlaying && (
            <span className="text-[10px] text-muted-foreground uppercase tracking-wide">
              speaking
            </span>
          )}
        </div>
        <div className="bg-muted/50 rounded-lg p-3 max-h-24 overflow-y-auto">
          <p className="text-sm text-foreground/80 leading-relaxed line-clamp-4">
            {currentSegment.text}
          </p>
        </div>
      </div>

      {/* Progress bar */}
      <div className="px-4 pb-1">
        <div
          className="w-full h-1 bg-muted rounded-full cursor-pointer"
          onClick={handleProgressClick}
        >
          <div
            className="h-full bg-primary rounded-full transition-[width] duration-100"
            style={{ width: `${progressPercent}%` }}
          />
        </div>
        <div className="flex justify-between mt-0.5">
          <span className="text-[10px] text-muted-foreground">
            {formatTime(globalTime)}
          </span>
          <span className="text-[10px] text-muted-foreground">
            {formatTime(manifest.total_duration_ms)}
          </span>
        </div>
      </div>

      {/* Controls */}
      <div className="px-4 pb-2 flex items-center justify-center gap-2">
        <Button variant="ghost" size="sm" onClick={handlePrev}>
          <SkipBack className="h-4 w-4" />
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={togglePlayPause}
          className="h-9 w-9 p-0 rounded-full"
        >
          {isPlaying ? (
            <Pause className="h-4 w-4" />
          ) : (
            <Play className="h-4 w-4 ml-0.5" />
          )}
        </Button>
        <Button variant="ghost" size="sm" onClick={handleNext}>
          <SkipForward className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleSpeedToggle}
          className="text-xs ml-2"
        >
          <Gauge className="h-3 w-3 mr-1" />
          {SPEED_OPTIONS[speedIndex]}x
        </Button>
      </div>

      {/* Speaker timeline */}
      <div className="px-4 pb-3">
        <SpeakerTimeline
          segments={segments}
          currentSegmentIndex={currentIndex}
          registry={registry}
          onSegmentClick={handleSegmentClick}
        />
        <div className="flex items-center gap-1 mt-1">
          <span className="text-[10px] text-muted-foreground">
            Round {currentSegment.round === 99 ? "Final" : currentSegment.round}
          </span>
          <span className="text-[10px] text-muted-foreground">
            &middot; {currentIndex + 1}/{segments.length}
          </span>
        </div>
      </div>
    </div>
  );
}
