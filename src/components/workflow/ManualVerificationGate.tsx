/**
 * Manual verification gate dialog (B-19 / Issue #176).
 *
 * Developer 가 `⚠️ Manual:` 로 열거한 항목들을 사용자에게 보여주고
 * pass/skip/fail 판정 + 선택적 fail 사유를 수집한다. 결과는 onComplete 로
 * items 와 동일 순서/길이 배열로 돌아간다.
 *
 * 상위 (예: DevProgressView impl-complete 핸들러) 가 이 dialog 를 띄워 결과를
 * startReviewRT 의 runManualGate 콜백에 넘긴다.
 */
import { useState, useMemo } from "react";
import { X, Check } from "lucide-react";
import { cn } from "@/lib/utils";
import type {
  ManualVerificationItem,
  ManualVerificationResult,
} from "@/lib/manualVerification";

type Status = "pass" | "skip" | "fail" | null;

interface ManualVerificationGateProps {
  open: boolean;
  items: ManualVerificationItem[];
  onComplete: (results: ManualVerificationResult[]) => void;
  onCancel: () => void;
}

const STATUS_LABEL: Record<Exclude<Status, null>, string> = {
  pass: "Pass",
  skip: "Skip",
  fail: "Fail",
};

const STATUS_CLS: Record<Exclude<Status, null>, string> = {
  pass: "bg-status-approved/20 text-status-approved border-status-approved/40",
  skip: "bg-muted/30 text-muted-foreground border-border/40",
  fail: "bg-destructive/20 text-destructive border-destructive/40",
};

export function ManualVerificationGate({
  open,
  items,
  onComplete,
  onCancel,
}: ManualVerificationGateProps) {
  const [statuses, setStatuses] = useState<Status[]>(() => items.map(() => null));
  const [reasons, setReasons] = useState<Record<number, string>>({});

  // items 가 바뀌면 상태 초기화 (다른 게이트 호출이 같은 dialog 인스턴스를 재사용할 때).
  useMemo(() => {
    setStatuses(items.map(() => null));
    setReasons({});
  }, [items]);

  if (!open) return null;

  const allSelected = statuses.every((s) => s !== null);

  const setAll = (status: "pass") => {
    setStatuses(items.map(() => status));
  };

  const setStatusAt = (idx: number, status: Exclude<Status, null>) => {
    setStatuses((prev) => prev.map((s, i) => (i === idx ? status : s)));
    if (status !== "fail") {
      setReasons((prev) => {
        const next = { ...prev };
        delete next[idx];
        return next;
      });
    }
  };

  const handleSubmit = () => {
    if (!allSelected) return;
    const results: ManualVerificationResult[] = statuses.map((s, i) => {
      const status = s as Exclude<Status, null>;
      const reason = status === "fail" ? reasons[i]?.trim() || undefined : undefined;
      return { status, reason };
    });
    onComplete(results);
  };

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/30 backdrop-blur-[1px]" onClick={onCancel} />

      <div className="relative bg-card border border-border/40 rounded-lg shadow-2xl w-[540px] max-h-[85vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 h-11 border-b border-border/30">
          <span className="text-[13px] font-medium text-foreground flex-1">
            수동 확인이 필요한 항목
          </span>
          <button
            onClick={onCancel}
            className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            title="취소"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 space-y-4">
          <p className="text-[11px] text-muted-foreground/70">
            Developer 가 직접 확인할 수 없어 사용자 확인을 요청했습니다. 각 항목을
            실제로 돌려보고 결과를 선택해 주세요. fail 시 실패 사유는 선택입니다.
          </p>

          <div className="space-y-3">
            {items.map((item, idx) => {
              const current = statuses[idx];
              return (
                <div
                  key={idx}
                  className="rounded-md border border-border/30 bg-background/50 p-3 space-y-2"
                >
                  <p className="text-[12px] text-foreground/90 leading-snug">{item.label}</p>
                  <div className="flex items-center gap-1.5">
                    {(["pass", "skip", "fail"] as const).map((s) => (
                      <button
                        key={s}
                        onClick={() => setStatusAt(idx, s)}
                        className={cn(
                          "px-2.5 py-1 rounded-md text-[11px] font-medium border transition-colors",
                          current === s
                            ? STATUS_CLS[s]
                            : "bg-accent/20 text-muted-foreground border-transparent hover:text-foreground/80"
                        )}
                      >
                        {STATUS_LABEL[s]}
                      </button>
                    ))}
                  </div>
                  {current === "fail" && (
                    <textarea
                      value={reasons[idx] ?? ""}
                      onChange={(e) =>
                        setReasons((prev) => ({ ...prev, [idx]: e.target.value }))
                      }
                      placeholder="실패 사유 (선택) — Developer rework 지시에 포함됩니다"
                      rows={2}
                      className="w-full bg-input rounded-md px-2.5 py-1.5 text-[11px] outline-none text-foreground placeholder:text-muted-foreground/40 border border-border/30 focus:border-ring/40 resize-none"
                    />
                  )}
                </div>
              );
            })}
          </div>

          <div className="flex items-center gap-2 pt-2 border-t border-border/20">
            <button
              onClick={() => setAll("pass")}
              className="px-2.5 py-1 rounded-md text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent/40 transition-colors"
            >
              모두 Pass
            </button>
            <span className="flex-1" />
            <button
              onClick={onCancel}
              className="px-3 py-1.5 rounded-md text-[11px] text-muted-foreground hover:bg-accent transition-colors"
            >
              취소
            </button>
            <button
              onClick={handleSubmit}
              disabled={!allSelected}
              className={cn(
                "flex items-center gap-1.5 px-4 py-1.5 rounded-md text-[11px] font-medium transition-colors",
                allSelected
                  ? "bg-primary/15 text-primary hover:bg-primary/25"
                  : "bg-muted/30 text-muted-foreground/40 cursor-not-allowed"
              )}
            >
              <Check className="w-3 h-3" />
              진행
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
