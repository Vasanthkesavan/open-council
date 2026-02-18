import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { appDataDir } from "@tauri-apps/api/path";
import { ArrowLeft, XCircle, Volume2, Pause, Play, X, Mic } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import DebateAgentMessage from "./DebateAgentMessage";
import DebateRoundHeader from "./DebateRoundHeader";
import DebateProgressBar from "./DebateProgressBar";
import ModeratorVerdict from "./ModeratorVerdict";
import AudioPlayer from "./AudioPlayer";
import AudioGenerationProgress from "./AudioGenerationProgress";
import AudioWaveform from "./AudioWaveform";
import { useLiveAudioQueue } from "@/hooks/useLiveAudioQueue";
import type { AgentMeta } from "@/lib/agentColors";
import { resolveAgentConfig } from "@/lib/agentColors";

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

interface DebateAudioRecord {
  id: string;
  decision_id: string;
  manifest_json: string;
  total_duration_ms: number;
  generated_at: string;
  audio_dir: string;
}

interface AudioProgressEvent {
  decision_id: string;
  completed: number;
  total: number;
  current_agent: string;
}

interface AudioCompleteEvent {
  decision_id: string;
  manifest: AudioManifest;
}

interface DebateRoundData {
  id: string;
  decision_id: string;
  round_number: number;
  exchange_number: number;
  agent: string;
  content: string;
  created_at: string;
}

interface AgentResponseEvent {
  decision_id: string;
  round_number: number;
  exchange_number: number;
  agent: string;
  content: string;
}

interface AgentTokenEvent {
  decision_id: string;
  round_number: number;
  exchange_number: number;
  agent: string;
  token: string;
}

interface PendingResponse {
  round_number: number;
  exchange_number: number;
  agent: string;
  content: string;
  received_at: number;
}

interface RoundCompleteEvent {
  decision_id: string;
  round_number: number;
  exchange_number: number;
}

const SYNC_TRANSCRIPT_WITH_LIVE_AUDIO = true;

interface DebateViewProps {
  decisionId: string;
  isDebating: boolean;
  quickMode: boolean;
  onBackToChat: () => void;
  onDebateComplete: () => void;
}

// Group rounds by (round_number, exchange_number)
interface RoundGroup {
  roundNumber: number;
  exchangeNumber: number;
  entries: DebateRoundData[];
}

function groupRounds(rounds: DebateRoundData[]): RoundGroup[] {
  const groups: RoundGroup[] = [];
  let current: RoundGroup | null = null;

  for (const r of rounds) {
    if (
      !current ||
      current.roundNumber !== r.round_number ||
      current.exchangeNumber !== r.exchange_number
    ) {
      current = {
        roundNumber: r.round_number,
        exchangeNumber: r.exchange_number,
        entries: [],
      };
      groups.push(current);
    }
    current.entries.push(r);
  }

  return groups;
}

export default function DebateView({
  decisionId,
  isDebating,
  quickMode,
  onBackToChat,
  onDebateComplete,
}: DebateViewProps) {
  const [rounds, setRounds] = useState<DebateRoundData[]>([]);
  const [currentRound, setCurrentRound] = useState(1);
  const [collapsedRounds, setCollapsedRounds] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [debateRunning, setDebateRunning] = useState(isDebating);
  const scrollEndRef = useRef<HTMLDivElement>(null);
  // Streaming text displayed in the UI (either token stream or audio-synced reveal)
  const [streamingMessages, setStreamingMessages] = useState<Record<string, { round_number: number; exchange_number: number; agent: string; content: string }>>(
    {}
  );
  const [pendingResponses, setPendingResponses] = useState<Record<string, PendingResponse>>({});
  const pendingResponsesRef = useRef<Record<string, PendingResponse>>({});
  const revealTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const liveAudioPlayingRef = useRef(false);
  const lastRevealedSegmentRef = useRef(-1);
  const sawLiveAudioRef = useRef(false);
  const [registry, setRegistry] = useState<AgentMeta[]>([]);

  // Audio playback state
  const [audioManifest, setAudioManifest] = useState<AudioManifest | null>(null);
  const [audioDir, setAudioDir] = useState<string>("");
  const [showPlayer, setShowPlayer] = useState(false);
  const [audioGenerating, setAudioGenerating] = useState(false);
  const [audioProgress, setAudioProgress] = useState({ completed: 0, total: 0, currentAgent: "" });

  // Live audio queue — plays TTS segments as they arrive during debate
  const liveAudio = useLiveAudioQueue(decisionId, debateRunning);

  // Total rounds: quick mode = 2 (round 1 + moderator), full = 5 (r1, r2e1, r2e2, r3, moderator)
  const totalRounds = quickMode ? 2 : 5;

  useEffect(() => {
    liveAudioPlayingRef.current = liveAudio.isPlaying;
  }, [liveAudio.isPlaying]);

  useEffect(() => {
    pendingResponsesRef.current = pendingResponses;
  }, [pendingResponses]);

  // Load existing debate data, agent registry, and audio on mount
  useEffect(() => {
    if (revealTimerRef.current) {
      clearInterval(revealTimerRef.current);
      revealTimerRef.current = null;
    }
    lastRevealedSegmentRef.current = -1;
    sawLiveAudioRef.current = false;
    pendingResponsesRef.current = {};
    setPendingResponses({});
    setStreamingMessages({});
    loadDebate();
    invoke<AgentMeta[]>("get_agent_registry").then(setRegistry).catch(console.error);
    loadExistingAudio();
  }, [decisionId]);

  async function loadExistingAudio() {
    try {
      const dataDir = await appDataDir();
      setAudioDir(`${dataDir}/debates/${decisionId}`);

      const record = await invoke<DebateAudioRecord | null>("get_debate_audio", { decisionId });
      if (record) {
        const manifest: AudioManifest = JSON.parse(record.manifest_json);
        setAudioManifest(manifest);
        if (record.audio_dir.includes("/") || record.audio_dir.includes("\\")) {
          setAudioDir(record.audio_dir);
        }
      }
    } catch {
      // No audio yet — that's fine
    }
  }
  // Listen for debate events
  useEffect(() => {
    // Listen for streaming tokens
    const unlistenToken = listen<AgentTokenEvent>(
      "debate-agent-token",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;
        const { round_number, exchange_number, agent, token } = event.payload;

        if (!SYNC_TRANSCRIPT_WITH_LIVE_AUDIO) {
          const key = `${round_number}-${exchange_number}-${agent}`;
          setStreamingMessages((prev) => {
            const existing = prev[key];
            return {
              ...prev,
              [key]: {
                round_number,
                exchange_number,
                agent,
                content: (existing?.content || "") + token,
              },
            };
          });
        }

        // Track current round for progress
        if (round_number !== 99) {
          setCurrentRound(round_number);
        } else {
          setCurrentRound(99);
        }
      }
    );

    // When a complete agent response arrives
    const unlistenAgentResponse = listen<AgentResponseEvent>(
      "debate-agent-response",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;
        const { round_number, exchange_number, agent, content } = event.payload;
        const streamKey = `${round_number}-${exchange_number}-${agent}`;

        if (round_number !== 99) {
          setCurrentRound(round_number);
        } else {
          setCurrentRound(99);
        }

        if (SYNC_TRANSCRIPT_WITH_LIVE_AUDIO) {
          setStreamingMessages((prev) => {
            const next = { ...prev };
            delete next[streamKey];
            return next;
          });
          setPendingResponses((prev) => {
            const next = {
              ...prev,
              [streamKey]: {
                round_number,
                exchange_number,
                agent,
                content,
                received_at: Date.now(),
              },
            };
            pendingResponsesRef.current = next;
            return next;
          });
        } else {
          setStreamingMessages((prev) => {
            const next = { ...prev };
            delete next[streamKey];
            return next;
          });

          setRounds((prev) => [
            ...prev,
            {
              id: `${round_number}-${exchange_number}-${agent}-${Date.now()}`,
              decision_id: decisionId,
              round_number,
              exchange_number,
              agent,
              content,
              created_at: new Date().toISOString(),
            },
          ]);
        }
      }
    );

    const unlistenRoundComplete = listen<RoundCompleteEvent>(
      "debate-round-complete",
      (_event) => {
        // Round complete events can be used for auto-collapse if desired
      }
    );

    const unlistenComplete = listen<{ decision_id: string }>(
      "debate-complete",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;
        setDebateRunning(false);
        onDebateComplete();
      }
    );

    const unlistenError = listen<{ decision_id: string; error: string }>(
      "debate-error",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;
        setDebateRunning(false);
        if (event.payload.error !== "Debate cancelled") {
          setError(event.payload.error);
        }
      }
    );

    // Audio generation events
    const unlistenAudioProgress = listen<AudioProgressEvent>(
      "audio-generation-progress",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;
        const { completed, total, current_agent } = event.payload;
        if (completed < total) {
          setAudioGenerating(true);
          setAudioProgress({ completed, total, currentAgent: current_agent });
        } else {
          setAudioGenerating(false);
        }
      }
    );

    const unlistenAudioComplete = listen<AudioCompleteEvent>(
      "audio-generation-complete",
      async (event) => {
        if (event.payload.decision_id !== decisionId) return;
        setAudioGenerating(false);
        setAudioManifest(event.payload.manifest);
        // Load audio dir from the saved record
        try {
          const record = await invoke<DebateAudioRecord | null>("get_debate_audio", { decisionId });
          if (record && (record.audio_dir.includes("/") || record.audio_dir.includes("\\"))) {
            setAudioDir(record.audio_dir);
          }
        } catch {
          // Use default path
        }
        setShowPlayer(true);
      }
    );

    const unlistenAudioError = listen<{ decision_id: string; error: string }>(
      "audio-generation-error",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;
        setAudioGenerating(false);
      }
    );

    return () => {
      unlistenToken.then((fn) => fn());
      unlistenAgentResponse.then((fn) => fn());
      unlistenRoundComplete.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      unlistenError.then((fn) => fn());
      unlistenAudioProgress.then((fn) => fn());
      unlistenAudioComplete.then((fn) => fn());
      unlistenAudioError.then((fn) => fn());
    };
  }, [decisionId, onDebateComplete]);

  useEffect(() => {
    if (liveAudio.segmentsReady > 0) {
      sawLiveAudioRef.current = true;
    }
  }, [liveAudio.segmentsReady]);

  // Reveal transcript only when the corresponding audio segment starts playing.
  useEffect(() => {
    if (!SYNC_TRANSCRIPT_WITH_LIVE_AUDIO) return;
    const segment = liveAudio.currentSegment;
    if (!segment) return;
    if (lastRevealedSegmentRef.current === segment.index) return;
    lastRevealedSegmentRef.current = segment.index;

    if (segment.roundNumber !== 99) {
      setCurrentRound(segment.roundNumber);
    } else {
      setCurrentRound(99);
    }

    const key = `${segment.roundNumber}-${segment.exchangeNumber}-${segment.agent}`;
    const pending = pendingResponsesRef.current[key];
    const fullText = pending?.content || segment.text || "";
    if (!fullText) return;

    if (pending) {
      const nextPending = { ...pendingResponsesRef.current };
      delete nextPending[key];
      pendingResponsesRef.current = nextPending;
      setPendingResponses(nextPending);
    }

    if (revealTimerRef.current) {
      clearInterval(revealTimerRef.current);
      revealTimerRef.current = null;
    }

    const durationMs = Math.max(900, segment.durationMs || 0);
    const tickMs = 35;
    const totalChars = fullText.length;
    const charsPerTick = Math.max(1, Math.ceil(totalChars / (durationMs / tickMs)));
    let shownChars = 0;

    setStreamingMessages((prev) => ({
      ...prev,
      [key]: {
        round_number: segment.roundNumber,
        exchange_number: segment.exchangeNumber,
        agent: segment.agent,
        content: "",
      },
    }));

    revealTimerRef.current = setInterval(() => {
      if (!liveAudioPlayingRef.current) return;

      shownChars = Math.min(totalChars, shownChars + charsPerTick);
      const partial = fullText.slice(0, shownChars);

      setStreamingMessages((prev) => ({
        ...prev,
        [key]: {
          round_number: segment.roundNumber,
          exchange_number: segment.exchangeNumber,
          agent: segment.agent,
          content: partial,
        },
      }));

      if (shownChars >= totalChars) {
        if (revealTimerRef.current) {
          clearInterval(revealTimerRef.current);
          revealTimerRef.current = null;
        }
        setStreamingMessages((prev) => {
          const next = { ...prev };
          delete next[key];
          return next;
        });
        setRounds((prev) => [
          ...prev,
          {
            id: `${segment.roundNumber}-${segment.exchangeNumber}-${segment.agent}-${Date.now()}`,
            decision_id: decisionId,
            round_number: segment.roundNumber,
            exchange_number: segment.exchangeNumber,
            agent: segment.agent,
            content: fullText,
            created_at: new Date().toISOString(),
          },
        ]);
      }
    }, tickMs);
  }, [decisionId, liveAudio.currentSegmentIndex, liveAudio.currentSegment]);

  // Fallback: if debate ends and some pending responses never got audio events,
  // flush them to the transcript so content is never lost.
  useEffect(() => {
    if (!SYNC_TRANSCRIPT_WITH_LIVE_AUDIO) return;
    if (debateRunning) return;
    if (Object.keys(pendingResponses).length === 0) return;

    const delayMs = sawLiveAudioRef.current ? 1500 : 0;
    const flushTimer = setTimeout(() => {
      const remaining = Object.values(pendingResponsesRef.current);
      if (remaining.length === 0) return;

      remaining.sort((a, b) => {
        if (a.round_number !== b.round_number) {
          return a.round_number - b.round_number;
        }
        if (a.exchange_number !== b.exchange_number) {
          return a.exchange_number - b.exchange_number;
        }
        return a.received_at - b.received_at;
      });

      setRounds((prev) => [
        ...prev,
        ...remaining.map((item) => ({
          id: `${item.round_number}-${item.exchange_number}-${item.agent}-${item.received_at}`,
          decision_id: decisionId,
          round_number: item.round_number,
          exchange_number: item.exchange_number,
          agent: item.agent,
          content: item.content,
          created_at: new Date(item.received_at).toISOString(),
        })),
      ]);

      setStreamingMessages((prev) => {
        const next = { ...prev };
        for (const item of remaining) {
          delete next[`${item.round_number}-${item.exchange_number}-${item.agent}`];
        }
        return next;
      });

      pendingResponsesRef.current = {};
      setPendingResponses({});
    }, delayMs);

    return () => clearTimeout(flushTimer);
  }, [debateRunning, decisionId, pendingResponses]);

  useEffect(() => {
    return () => {
      if (revealTimerRef.current) {
        clearInterval(revealTimerRef.current);
        revealTimerRef.current = null;
      }
    };
  }, []);

  // Auto-scroll as new content arrives
  useEffect(() => {
    scrollEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [rounds, streamingMessages]);

  async function loadDebate() {
    try {
      const data = await invoke<DebateRoundData[]>("get_debate", {
        decisionId,
      });
      setRounds(data);

      // Determine current round from loaded data
      if (data.length > 0) {
        const maxRound = Math.max(...data.map((r) => r.round_number));
        setCurrentRound(maxRound);
        if (maxRound === 99) {
          setDebateRunning(false);
        }
      }
    } catch (err) {
      console.error("Failed to load debate:", err);
    }
  }

  async function handleCancel() {
    try {
      await invoke("cancel_debate", { decisionId });
      setDebateRunning(false);
    } catch (err) {
      console.error("Failed to cancel debate:", err);
    }
  }

  function toggleRound(key: string) {
    setCollapsedRounds((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  const groups = groupRounds(rounds);

  // Merge streaming messages into display groups
  const streamingEntries = Object.entries(streamingMessages);
  const streamingByRound: Record<string, typeof streamingEntries> = {};
  for (const entry of streamingEntries) {
    const [, msg] = entry;
    const key = `${msg.round_number}-${msg.exchange_number}`;
    if (!streamingByRound[key]) streamingByRound[key] = [];
    streamingByRound[key].push(entry);
  }

  // Collect all round keys (finalized + streaming-only)
  const allRoundKeys = new Set<string>();
  for (const g of groups) allRoundKeys.add(`${g.roundNumber}-${g.exchangeNumber}`);
  for (const key of Object.keys(streamingByRound)) allRoundKeys.add(key);

  // Build merged groups: finalized rounds + streaming-only rounds
  const mergedGroups: (RoundGroup & { streamingEntries?: typeof streamingEntries })[] = groups.map((g) => ({
    ...g,
    streamingEntries: streamingByRound[`${g.roundNumber}-${g.exchangeNumber}`],
  }));

  // Add streaming-only rounds (no finalized entries yet)
  for (const key of Object.keys(streamingByRound)) {
    if (!groups.some((g) => `${g.roundNumber}-${g.exchangeNumber}` === key)) {
      const [, first] = streamingByRound[key][0];
      mergedGroups.push({
        roundNumber: first.round_number,
        exchangeNumber: first.exchange_number,
        entries: [],
        streamingEntries: streamingByRound[key],
      });
    }
  }

  // Sort merged groups
  mergedGroups.sort((a, b) => a.roundNumber - b.roundNumber || a.exchangeNumber - b.exchangeNumber);

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      {/* Progress bar */}
      <DebateProgressBar
        currentRound={currentRound}
        totalRounds={totalRounds}
        isRunning={debateRunning}
        quickMode={quickMode}
      />

      {/* Debate content */}
      <ScrollArea className="flex-1">
        <div className="max-w-3xl mx-auto px-4 py-3">
          {mergedGroups.length === 0 && debateRunning && (
            <div className="text-center text-muted-foreground py-12">
              <p className="text-sm">Preparing the committee...</p>
            </div>
          )}

          {mergedGroups.map((group) => {
            const key = `${group.roundNumber}-${group.exchangeNumber}`;
            const isCollapsed = collapsedRounds.has(key);
            const isActive =
              debateRunning && group.roundNumber === currentRound;
            const isModerator = group.roundNumber === 99;

            return (
              <div key={key}>
                <DebateRoundHeader
                  roundNumber={group.roundNumber}
                  exchangeNumber={group.exchangeNumber}
                  isActive={isActive}
                  isCollapsed={isCollapsed}
                  onToggle={() => toggleRound(key)}
                />
                {!isCollapsed && (
                  <div className="ml-1 mb-2">
                    {/* Finalized entries */}
                    {group.entries.map((entry) =>
                      isModerator ? (
                        <ModeratorVerdict
                          key={entry.id}
                          content={entry.content}
                        />
                      ) : (
                        <DebateAgentMessage
                          key={entry.id}
                          agent={entry.agent}
                          content={entry.content}
                          registry={registry}
                        />
                      )
                    )}
                    {/* Streaming entries (in-progress) */}
                    {group.streamingEntries?.map(([streamKey, msg]) =>
                      msg.agent === "moderator" ? (
                        <ModeratorVerdict
                          key={`stream-${streamKey}`}
                          content={msg.content}
                        />
                      ) : (
                        <DebateAgentMessage
                          key={`stream-${streamKey}`}
                          agent={msg.agent}
                          content={msg.content}
                          isStreaming
                          registry={registry}
                        />
                      )
                    )}
                  </div>
                )}
              </div>
            );
          })}

          {error && (
            <div className="px-4 py-3 rounded-lg bg-destructive/20 border border-destructive/30 text-destructive text-sm mt-4">
              {error}
            </div>
          )}

          <div ref={scrollEndRef} />
        </div>
      </ScrollArea>

      {/* Live audio indicator — plays TTS as debate streams */}
      {debateRunning && liveAudio.segmentsReady > 0 && (
        <div className="border-t border-border px-4 py-2 flex items-center gap-3">
          {liveAudio.currentAgent ? (
            <>
              {(() => {
                const config = resolveAgentConfig(liveAudio.currentAgent, registry);
                const meta = registry.find((a) => a.key === liveAudio.currentAgent);
                return (
                  <>
                    <span className="text-sm">{config.emoji}</span>
                    {meta && (
                      <AudioWaveform
                        isActive={liveAudio.isPlaying}
                        color={meta.color}
                        size="sm"
                      />
                    )}
                    <span className={`text-xs font-medium ${config.color}`}>
                      {config.label}
                    </span>
                  </>
                );
              })()}
            </>
          ) : (
            <span className="text-xs text-muted-foreground">
              Generating audio...
            </span>
          )}
          <div className="ml-auto flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={liveAudio.togglePause}
              className="h-7 w-7 p-0"
            >
              {liveAudio.isPlaying ? (
                <Pause className="h-3.5 w-3.5" />
              ) : (
                <Play className="h-3.5 w-3.5" />
              )}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={liveAudio.stop}
              className="h-7 w-7 p-0"
            >
              <X className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      )}

      {/* Audio generation progress (for manual re-generation) */}
      {audioGenerating && (
        <div className="px-4 py-2 border-t border-border">
          <AudioGenerationProgress
            completed={audioProgress.completed}
            total={audioProgress.total}
            currentAgent={audioProgress.currentAgent}
            registry={registry}
          />
        </div>
      )}

      {/* Audio player (for post-debate replay) */}
      {showPlayer && audioManifest && audioDir && (
        <AudioPlayer
          manifest={audioManifest}
          audioDir={audioDir}
          registry={registry}
          onClose={() => setShowPlayer(false)}
        />
      )}

      {/* Bottom action bar */}
      <div className="border-t border-border px-4 py-2 flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBackToChat}>
          <ArrowLeft className="h-3.5 w-3.5 mr-1.5" />
          Back to Chat
        </Button>
        {!debateRunning && audioManifest && !showPlayer && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowPlayer(true)}
            className="ml-auto"
          >
            <Volume2 className="h-3.5 w-3.5 mr-1.5" />
            Listen to Debate
          </Button>
        )}
        {!debateRunning && !audioManifest && !audioGenerating && rounds.length > 0 && (
          <Button
            variant="outline"
            size="sm"
            onClick={async () => {
              try {
                setAudioGenerating(true);
                await invoke("generate_debate_audio", { decisionId });
              } catch (err) {
                console.error("Failed to generate audio:", err);
                setAudioGenerating(false);
              }
            }}
            className="ml-auto"
          >
            <Mic className="h-3.5 w-3.5 mr-1.5" />
            Generate Audio
          </Button>
        )}
        {debateRunning && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleCancel}
            className="ml-auto text-destructive hover:text-destructive"
          >
            <XCircle className="h-3.5 w-3.5 mr-1.5" />
            Cancel Debate
          </Button>
        )}
      </div>
    </div>
  );
}

