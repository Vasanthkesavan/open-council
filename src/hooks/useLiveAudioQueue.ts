import { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";

interface SegmentAudioReadyEvent {
  decision_id: string;
  segment_index: number;
  agent: string;
  round_number: number;
  exchange_number: number;
  audio_file: string;
  duration_ms: number;
  audio_dir: string;
}

export interface LiveAudioState {
  isPlaying: boolean;
  currentSegmentIndex: number;
  currentAgent: string | null;
  segmentsReady: number;
  togglePause: () => void;
  stop: () => void;
}

export function useLiveAudioQueue(
  decisionId: string,
  isDebating: boolean
): LiveAudioState {
  const readySegments = useRef<Map<number, SegmentAudioReadyEvent>>(new Map());
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const nextToPlay = useRef(0);
  const isPlayingRef = useRef(false);
  const userPaused = useRef(false);

  const [currentSegmentIndex, setCurrentSegmentIndex] = useState(-1);
  const [currentAgent, setCurrentAgent] = useState<string | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [segmentsReady, setSegmentsReady] = useState(0);

  // Initialize Audio element once
  useEffect(() => {
    audioRef.current = new Audio();
    return () => {
      if (audioRef.current) {
        audioRef.current.pause();
        audioRef.current.src = "";
        audioRef.current = null;
      }
    };
  }, []);

  // Try to play the next segment in queue
  const tryPlayNext = useCallback(() => {
    if (userPaused.current) return;

    const idx = nextToPlay.current;
    const segment = readySegments.current.get(idx);
    if (!segment) return; // Not ready yet — will retry when event arrives

    const audio = audioRef.current;
    if (!audio) return;

    const filePath = `${segment.audio_dir}/${segment.audio_file}`.replace(
      /\\/g,
      "/"
    );
    const url = convertFileSrc(filePath);
    audio.src = url;
    audio.load();
    audio.play().catch(console.error);

    isPlayingRef.current = true;
    setIsPlaying(true);
    setCurrentSegmentIndex(idx);
    setCurrentAgent(segment.agent);
  }, []);

  // When audio ends, advance to next segment
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const handleEnded = () => {
      nextToPlay.current += 1;
      // Small gap between speakers
      setTimeout(() => tryPlayNext(), 500);
    };

    audio.addEventListener("ended", handleEnded);
    return () => audio.removeEventListener("ended", handleEnded);
  }, [tryPlayNext]);

  // Listen for segment-ready events
  useEffect(() => {
    const unlisten = listen<SegmentAudioReadyEvent>(
      "debate-segment-audio-ready",
      (event) => {
        if (event.payload.decision_id !== decisionId) return;

        readySegments.current.set(
          event.payload.segment_index,
          event.payload
        );
        setSegmentsReady(readySegments.current.size);

        // If this is the segment we're waiting to play, start playing
        if (event.payload.segment_index === nextToPlay.current) {
          tryPlayNext();
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [decisionId, tryPlayNext]);

  // Reset when a new debate starts
  useEffect(() => {
    if (isDebating) {
      readySegments.current.clear();
      nextToPlay.current = 0;
      userPaused.current = false;
      isPlayingRef.current = false;
      setIsPlaying(false);
      setCurrentSegmentIndex(-1);
      setCurrentAgent(null);
      setSegmentsReady(0);
    }
  }, [isDebating]);

  const togglePause = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;

    if (isPlayingRef.current) {
      audio.pause();
      isPlayingRef.current = false;
      userPaused.current = true;
      setIsPlaying(false);
    } else {
      userPaused.current = false;
      if (audio.src) {
        audio.play().catch(console.error);
        isPlayingRef.current = true;
        setIsPlaying(true);
      } else {
        // No current source — try to play the next queued segment
        tryPlayNext();
      }
    }
  }, [tryPlayNext]);

  const stop = useCallback(() => {
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      audio.src = "";
    }
    isPlayingRef.current = false;
    userPaused.current = true;
    setIsPlaying(false);
    setCurrentAgent(null);
  }, []);

  return {
    isPlaying,
    currentSegmentIndex,
    currentAgent,
    segmentsReady,
    togglePause,
    stop,
  };
}
