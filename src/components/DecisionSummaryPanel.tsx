import { cn } from "@/lib/utils";
import { CheckCircle2, AlertTriangle, TrendingUp, Lightbulb } from "lucide-react";
import { Button } from "@/components/ui/button";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import CommitteeVoteTally from "./CommitteeVoteTally";
import type { AgentMeta } from "@/lib/agentColors";

interface Option {
  label: string;
  description?: string;
}

interface Variable {
  label: string;
  value: string;
  impact?: "high" | "medium" | "low";
}

interface ProsCons {
  option: string;
  pros?: string[];
  cons?: string[];
  alignment_score?: number;
  alignment_reasoning?: string;
}

interface Recommendation {
  choice: string;
  confidence: "high" | "medium" | "low";
  reasoning: string;
  tradeoffs?: string;
  next_steps?: string[];
}

interface DebateSummary {
  consensus_points?: string[];
  key_disagreements?: string[];
  biases_identified?: string[];
  final_votes?: Record<string, string>;
}

export interface DecisionSummary {
  options?: Option[];
  variables?: Variable[];
  pros_cons?: ProsCons[];
  recommendation?: Recommendation;
  debate_summary?: DebateSummary;
}

interface DecisionSummaryPanelProps {
  summary: DecisionSummary | null;
  status: string;
  userChoice?: string | null;
  userChoiceReasoning?: string | null;
  outcome?: string | null;
  outcomeDate?: string | null;
  registry?: AgentMeta[];
  onAcceptRecommendation?: () => void;
  onChoseDifferently?: () => void;
  onNeedMoreTime?: () => void;
  onLogOutcome?: () => void;
  onReopen?: () => void;
  onCancelDebate?: () => void;
}

const IMPACT_COLORS: Record<string, string> = {
  high: "bg-red-500/20 text-red-400 border-red-500/30",
  medium: "bg-amber-500/20 text-amber-400 border-amber-500/30",
  low: "bg-muted text-muted-foreground border-border",
};

const CONFIDENCE_COLORS: Record<string, string> = {
  high: "bg-green-500/20 text-green-400",
  medium: "bg-amber-500/20 text-amber-400",
  low: "bg-red-500/20 text-red-400",
};

function AlignmentBar({ score }: { score: number }) {
  const color =
    score >= 7 ? "bg-green-500" : score >= 4 ? "bg-amber-500" : "bg-red-500";
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-2 bg-muted rounded-full overflow-hidden">
        <div
          className={cn("h-full rounded-full transition-all", color)}
          style={{ width: `${(score / 10) * 100}%` }}
        />
      </div>
      <span className="text-xs text-muted-foreground font-medium w-6 text-right">
        {score}/10
      </span>
    </div>
  );
}

function normalizeMarkdown(content: string): string {
  return !content.includes("\n") && content.includes("\\n")
    ? content.replace(/\\r\\n/g, "\n").replace(/\\n/g, "\n")
    : content;
}

const markdownTextClasses =
  "text-muted-foreground leading-relaxed [&_p]:my-1.5 [&_strong]:font-semibold [&_a]:text-blue-600 dark:[&_a]:text-blue-400 [&_ul]:my-1.5 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:my-1.5 [&_ol]:list-decimal [&_ol]:pl-5 [&_li]:my-0.5 [&_li>p]:my-0 [&_pre]:my-2 [&_pre]:rounded-lg [&_pre]:border [&_pre]:border-border [&_pre]:bg-background [&_pre]:p-2.5 [&_pre]:overflow-x-auto [&_code:not(pre_code)]:rounded [&_code:not(pre_code)]:bg-muted/50 [&_code:not(pre_code)]:px-1 [&_code:not(pre_code)]:py-0.5 [&_code]:text-foreground/85";

const markdownInlineClasses =
  "text-foreground leading-snug [&_p]:m-0 [&_strong]:font-semibold [&_em]:italic [&_code]:text-foreground/85";

export default function DecisionSummaryPanel({
  summary,
  status,
  userChoice,
  userChoiceReasoning,
  outcome,
  outcomeDate,
  registry = [],
  onAcceptRecommendation,
  onChoseDifferently,
  onNeedMoreTime,
  onLogOutcome,
  onReopen,
  onCancelDebate,
}: DecisionSummaryPanelProps) {
  const shouldShowActions =
    status === "debating" || status === "recommended" || status === "decided" || status === "reviewed";

  const formattedOutcomeDate = outcomeDate
    ? new Date(outcomeDate).toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
      })
    : null;

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 overflow-y-auto overflow-x-hidden p-4 space-y-4">
        {!summary && (
          <div className="h-full flex items-center justify-center p-6">
            <div className="text-center text-muted-foreground">
              <Lightbulb className="h-8 w-8 mx-auto mb-3 opacity-40" />
              <p className="text-sm">
                The decision summary will appear here as the conversation progresses.
              </p>
            </div>
          </div>
        )}
        {summary && (
          <>
            {/* Options */}
            {summary.options && summary.options.length > 0 && (
              <section>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Options
                </h3>
                <div className="space-y-2">
                  {summary.options.map((opt, i) => (
                    <div
                      key={i}
                      className="p-3 rounded-lg border border-border bg-muted/30"
                    >
                      <div className="font-medium text-sm">{opt.label}</div>
                      {opt.description && (
                        <div className="text-xs text-muted-foreground mt-1">
                          {opt.description}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </section>
            )}

            {/* Key Variables */}
            {summary.variables && summary.variables.length > 0 && (
              <section>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Key Variables
                </h3>
                <div className="space-y-1.5">
                  {summary.variables.map((v, i) => {
                    // Normalize impact to high/medium/low
                    const impactKey = v.impact && ["high", "medium", "low"].includes(v.impact)
                      ? v.impact
                      : null;
                    return (
                      <div
                        key={i}
                        className="flex items-start justify-between gap-2 text-sm"
                      >
                        <div className="min-w-0 flex-1">
                          <span className="font-medium">{v.label}:</span>{" "}
                          <span className="text-muted-foreground">{v.value}</span>
                        </div>
                        {impactKey && (
                          <span
                            className={cn(
                              "text-[10px] font-semibold uppercase px-1.5 py-0.5 rounded border shrink-0",
                              IMPACT_COLORS[impactKey]
                            )}
                          >
                            {impactKey}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </section>
            )}

            {/* Analysis / Pros & Cons */}
            {summary.pros_cons && summary.pros_cons.length > 0 && (
              <section>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Analysis
                </h3>
                <div className="space-y-3">
                  {summary.pros_cons.map((pc, i) => (
                    <div
                      key={i}
                      className="p-3 rounded-lg border border-border bg-muted/20"
                    >
                      <div className="font-medium text-sm mb-2">{pc.option}</div>

                      {pc.alignment_score !== undefined && (
                        <div className="mb-2">
                          <AlignmentBar score={pc.alignment_score} />
                        </div>
                      )}

                      {pc.pros && pc.pros.length > 0 && (
                        <div className="mb-1.5">
                          <div className="text-xs text-green-400 font-medium mb-0.5 flex items-center gap-1">
                            <TrendingUp className="h-3 w-3" /> Pros
                          </div>
                          <ul className="text-xs text-muted-foreground space-y-0.5 ml-4">
                            {pc.pros.map((p, j) => (
                              <li key={j} className="list-disc">
                                {p}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {pc.cons && pc.cons.length > 0 && (
                        <div className="mb-1.5">
                          <div className="text-xs text-red-400 font-medium mb-0.5 flex items-center gap-1">
                            <AlertTriangle className="h-3 w-3" /> Cons
                          </div>
                          <ul className="text-xs text-muted-foreground space-y-0.5 ml-4">
                            {pc.cons.map((c, j) => (
                              <li key={j} className="list-disc">
                                {c}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {pc.alignment_reasoning && (
                        <div className="text-xs text-muted-foreground/70 italic mt-1">
                          {pc.alignment_reasoning}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </section>
            )}

            {/* Recommendation */}
            {summary.recommendation && (
              <section>
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Recommendation
                </h3>
                <div className="p-4 rounded-lg border-2 border-primary/30 bg-primary/5">
                  <div className="flex items-center justify-between mb-2 gap-2">
                    <div className="font-semibold text-sm flex items-start gap-2 min-w-0">
                      <CheckCircle2 className="h-4 w-4 text-primary shrink-0" />
                      <div className={cn("min-w-0 flex-1", markdownInlineClasses)}>
                        <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]}>
                          {normalizeMarkdown(summary.recommendation.choice)}
                        </ReactMarkdown>
                      </div>
                    </div>
                    <span
                      className={cn(
                        "text-[10px] font-semibold uppercase px-2 py-0.5 rounded shrink-0",
                        CONFIDENCE_COLORS[summary.recommendation.confidence] ||
                          CONFIDENCE_COLORS.medium
                      )}
                    >
                      {summary.recommendation.confidence} confidence
                    </span>
                  </div>

                  <div className={cn("text-xs mb-2", markdownTextClasses)}>
                    <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]}>
                      {normalizeMarkdown(summary.recommendation.reasoning)}
                    </ReactMarkdown>
                  </div>

                  {summary.recommendation.tradeoffs && (
                    <div className="text-xs mt-2">
                      <div className="font-medium text-amber-400 mb-1">Tradeoffs:</div>
                      <div className={markdownTextClasses}>
                        <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]}>
                          {normalizeMarkdown(summary.recommendation.tradeoffs)}
                        </ReactMarkdown>
                      </div>
                    </div>
                  )}

                  {summary.recommendation.next_steps &&
                    summary.recommendation.next_steps.length > 0 && (
                      <div className="mt-3">
                        <div className="text-xs font-medium mb-1">Next Steps:</div>
                        <ul className="text-xs text-muted-foreground space-y-2">
                          {summary.recommendation.next_steps.map((step, i) => (
                            <li key={i} className="flex items-start gap-2">
                              <span className="mt-0.5 h-3.5 w-3.5 rounded-sm border border-border bg-background/70 shrink-0" />
                              <div className={cn("min-w-0 flex-1", markdownTextClasses)}>
                                <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]}>
                                  {normalizeMarkdown(step)}
                                </ReactMarkdown>
                              </div>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                </div>
              </section>
            )}
          </>
        )}

        {/* Debate Summary — committee votes and highlights */}
        {summary?.debate_summary && (
          <>
            {summary.debate_summary.final_votes &&
              Object.keys(summary.debate_summary.final_votes).length > 0 && (
                <CommitteeVoteTally votes={summary.debate_summary.final_votes} registry={registry} />
              )}

            {summary.debate_summary.consensus_points &&
              summary.debate_summary.consensus_points.length > 0 && (
                <section>
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                    Committee Consensus
                  </h3>
                  <ul className="text-xs text-muted-foreground space-y-1 ml-3">
                    {summary.debate_summary.consensus_points.map((pt, i) => (
                      <li key={i} className="list-disc">{pt}</li>
                    ))}
                  </ul>
                </section>
              )}

            {summary.debate_summary.biases_identified &&
              summary.debate_summary.biases_identified.length > 0 && (
                <section>
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                    Biases Identified
                  </h3>
                  <ul className="text-xs text-amber-400/80 space-y-1 ml-3">
                    {summary.debate_summary.biases_identified.map((b, i) => (
                      <li key={i} className="list-disc">{b}</li>
                    ))}
                  </ul>
                </section>
              )}
          </>
        )}

        {status === "decided" && userChoice && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
              Your Decision
            </h3>
            <div className="p-3 rounded-lg border border-green-500/30 bg-green-500/10">
              <div className="text-sm font-medium text-foreground prose prose-sm dark:prose-invert max-w-none prose-p:my-0">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{userChoice}</ReactMarkdown>
              </div>
              {userChoiceReasoning && (
                <p className="text-xs text-muted-foreground mt-1 whitespace-pre-wrap">
                  {userChoiceReasoning}
                </p>
              )}
            </div>
          </section>
        )}

        {status === "reviewed" && outcome && (
          <section>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
              Outcome
            </h3>
            <div className="p-3 rounded-lg border border-border bg-muted/20">
              <p className="text-xs text-muted-foreground whitespace-pre-wrap">
                {outcome}
              </p>
              {formattedOutcomeDate && (
                <p className="text-[11px] text-muted-foreground/80 mt-2">
                  Logged on {formattedOutcomeDate}
                </p>
              )}
            </div>
          </section>
        )}
      </div>

      {shouldShowActions && (
        <div className="border-t border-border p-4 space-y-2 bg-background/95 backdrop-blur-sm">
          {status === "debating" && (
            <Button
              onClick={onCancelDebate}
              variant="ghost"
              className="w-full text-destructive hover:text-destructive"
            >
              Cancel Debate
            </Button>
          )}

          {status === "recommended" && (
            <>
              <Button onClick={onAcceptRecommendation} className="w-full">
                I'll go with this
              </Button>
              <Button
                onClick={onChoseDifferently}
                variant="outline"
                className="w-full"
              >
                I chose differently
              </Button>
              <Button onClick={onNeedMoreTime} variant="ghost" className="w-full">
                I need more time
              </Button>
            </>
          )}

          {status === "decided" && (
            <>
              <Button onClick={onLogOutcome} variant="outline" className="w-full">
                Log Outcome
              </Button>
              <Button onClick={onReopen} variant="ghost" className="w-full">
                Reopen
              </Button>
            </>
          )}

          {status === "reviewed" && (
            <Button onClick={onReopen} variant="ghost" className="w-full">
              Reopen
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
