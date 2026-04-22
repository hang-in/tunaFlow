# Subtask 03 — Stance-conflict detection (rule-first + small model verify + modal)

> 상위 plan: [userWorldviewInjectionPlan.md](./userWorldviewInjectionPlan.md)

## Changed files

- `src-tauri/src/commands/agents_helpers/send_common/stance_check.rs` (신규) — rule precheck (partial-shift 감지 포함) + model verify + `Unknown` fallback.
- `src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs` — stance-check 결과를 compact fragment 로 주입.
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` — prepare 단계에서 stance_check 호출 + 결과 저장.
- `src/lib/stanceConflictMarker.ts` (신규) — `<!-- tunaflow:stance-conflict:... -->` 마커 파서.
- `src/components/tunaflow/StanceConflictModal.tsx` (신규) — 사용자 confirmation UI.
- `src/components/tunaflow/chat/*` — 메시지 렌더 파이프라인에서 marker 감지 시 modal 트리거.

## Change description

### 1. Rule precheck (Rust, no model)

```rust
// src-tauri/src/commands/agents_helpers/send_common/stance_check.rs
pub enum PrecheckResult {
    NoConflict,
    Conflict { snapshot: PreferenceSnapshot, matched_keywords: Vec<String> },
    Ambiguous { candidates: Vec<PreferenceSnapshot> },
}

pub fn precheck(
    user_prompt: &str,
    snapshots: &[PreferenceSnapshot],
    recent_events: &[PreferenceEvent],
) -> PrecheckResult {
    let tokens = simple_tokenize(user_prompt);            // lowercase whitespace split
    let mut matched: Vec<(PreferenceSnapshot, Vec<String>)> = Vec::new();

    for snap in snapshots {
        // field 이름 + stance 값에서 keyword 추출
        let keywords = extract_keywords(&snap.field, &snap.current_stance);
        let overlap: Vec<_> = keywords.iter()
            .filter(|kw| tokens.contains(&kw.to_lowercase()))
            .cloned().collect();
        if !overlap.is_empty() {
            matched.push((snap.clone(), overlap));
        }
    }

    match matched.len() {
        0 => PrecheckResult::NoConflict,
        1 => {
            let (snap, kws) = &matched[0];
            // Codex review 2026-04-23 반영 — 부분 전환 silent pass 방지.
            // 단일 매칭이라도 partial-shift 키워드 있으면 Ambiguous 강제.
            if has_partial_shift_signal(user_prompt) {
                return PrecheckResult::Ambiguous { candidates: vec![snap.clone()] };
            }
            if contradicts_stance(user_prompt, snap) {
                PrecheckResult::Conflict { snapshot: snap.clone(), matched_keywords: kws.clone() }
            } else {
                PrecheckResult::NoConflict
            }
        }
        _ => PrecheckResult::Ambiguous {
            candidates: matched.into_iter().map(|(s, _)| s).collect(),
        },
    }
}

fn contradicts_stance(prompt: &str, snap: &PreferenceSnapshot) -> bool {
    // 부정어 휴리스틱:
    //   prompt 에 "바꾸", "말고", "대신", "포기", "새로", "change", "instead" 등이 있고
    //   current_stance 키워드와 함께 등장하면 contradicts=true
}

/// Codex review 2026-04-23 — 부분 전환 패턴 감지.
/// "CLI 유지하면서 SDK 실험만" 같은 partial-shift 요청을 NoConflict 로 잘못 판정하지 않도록
/// Ambiguous 로 escalate 하여 model verify 로 보낸다.
fn has_partial_shift_signal(prompt: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "유지하면서", "유지한 채", "실험만", "실험으로",
        "먼저", "병행", "같이", "도 해보",
        "only", "for now", "alongside", "try",
    ];
    let lower = prompt.to_lowercase();
    KEYWORDS.iter().any(|kw| lower.contains(*kw))
}
```

### 2. Model verify (ambiguous 케이스)

```rust
pub async fn verify_ambiguous(
    user_prompt: &str,
    candidates: &[PreferenceSnapshot],
    app: &AppHandle,
) -> Result<VerifyResult, AppError> {
    let small_model = std::env::var("TUNAFLOW_STANCE_VERIFY_MODEL")
        .unwrap_or_else(|_| "claude-haiku-4-5".to_string());

    let prompt = format!(
        "User preferences:\n{}\n\nUser request:\n{}\n\n\
         Answer in one line: CONFLICT with <field> or NO_CONFLICT. Do not explain.",
        candidates.iter().map(snap_to_line).collect::<Vec<_>>().join("\n"),
        user_prompt,
    );

    let output = tokio::time::timeout(
        Duration::from_secs(10),
        crate::agents::claude::run_one_shot(small_model, prompt),
    ).await;

    match output {
        Ok(Ok(resp)) => parse_verify_response(&resp.content),
        // Codex review 2026-04-23 반영 — timeout/error 를 conflict 로 처리하면
        // modal 피로도 폭증. Unknown 상태로 반환해 UI 가 soft-confirm (inline warning) 으로 표시.
        _ => VerifyResult::Unknown {
            reason: if matches!(output, Err(_)) { "timeout".into() } else { "model_error".into() },
        },
    }
}

/// UI 는 `Unknown` 에 대해 modal 을 띄우지 않는다. 대신 응답 상단에 inline warning:
///   "⚠ 선호도 검증 미완료 (model timeout) — 진행하되 사후 검토 권장"
/// 사용자가 "확인" 클릭 시 warning 닫힘. 별도 DB write 없음.
```

### 3. Compact fragment 주입

`prompt_assembly.rs`:

```rust
let stance_result = stance_check::run(&data.user_prompt, &snapshots, &recent_events, &app).await;
let stance_fragment = match stance_result {
    CheckResult::NoConflict => None,
    CheckResult::Conflict { snapshot, matched } =>
        Some(format!("⚠ Stance conflict detected: user previously stated `{}={}`. \
                      Current request may contradict. Ask for explicit confirmation.",
                     snapshot.field, snapshot.current_stance)),
    // model verify 결과는 동일 포맷
};
if let Some(frag) = stance_fragment {
    sections.insert(after_worldview_idx, ("stance_check", frag));
}
```

### 4. Marker-based modal

Agent 응답에 `<!-- tunaflow:stance-conflict:<memory_name>:<field>:<rationale> -->` 마커 출현 시 UI 가 intercept. 이 마커는 agent 가 자체 판단 (추가 명시적 거부권) 으로 낼 수도 있음 — rule precheck 주입된 경우에도 agent 가 "이건 실제 conflict 맞다" 며 재확인 요청 가능.

```ts
// src/lib/stanceConflictMarker.ts
// Codex review 2026-04-23 반영 — regex 를 하이픈 안전 non-greedy 로 교정 + composite key 파싱.

export function extractStanceConflict(text: string):
    { memoryName: string; field: string; rationale: string } | null {
    // 포맷: <!-- tunaflow:stance-conflict:<memory_name>:<field>:<rationale> -->
    // rationale 에 하이픈 허용 — `[\s\S]*?` non-greedy 로 `-->` 직전까지 매칭.
    const m = text.match(/<!--\s*tunaflow:stance-conflict:([^:]+):([^:]+):([\s\S]*?)\s*-->/);
    return m ? { memoryName: m[1], field: m[2], rationale: m[3].trim() } : null;
}

export function stripStanceConflictMarker(text: string): string {
    return text.replace(/<!--\s*tunaflow:stance-conflict:[\s\S]*?-->/g, "");
}
```

Agent 메시지 렌더 경로:
- streaming 완료 시 `extractStanceConflict` 호출
- match 있으면 StanceConflictModal 표시 + 메시지 본문은 `stripStanceConflictMarker` 로 clean 한 버전 렌더 (INV-6)

### 5. StanceConflictModal

```tsx
// Props 는 marker 의 composite key 그대로 수용.
type Props = {
    memoryName: string;
    field: string;
    rationale: string;
    onClose: (result: { action: 'confirm_change' | 'keep_existing' | 'ignore' }) => void;
};

export function StanceConflictModal({ memoryName, field, rationale, onClose }: Props) {
    const [snapshot, setSnapshot] = useState<PreferenceSnapshot | null>(null);
    useEffect(() => {
        // Subtask 02 에서 신설된 get_preference_snapshot command 사용 (Codex review 반영).
        invoke<PreferenceSnapshot | null>('get_preference_snapshot', { memoryName, field })
            .then(setSnapshot);
    }, [memoryName, field]);

    const confirmChange = async (newStance: string, reason: string) => {
        await invoke('record_user_preference_change', {
            memoryName: snapshot?.memory_name, field: snapshot?.field,
            stanceFrom: snapshot?.current_stance, stanceTo: newStance,
            reasonText: reason, reasonTags: []
        });
        onClose({ action: 'confirm_change' });
    };
    const keepExisting = () => onClose({ action: 'keep_existing' });
    const ignore = () => onClose({ action: 'ignore' });    // INV-5: no timeline write

    return (
        <Modal>
            <h3>Stance conflict 감지</h3>
            <p>이전 선호: <code>{snapshot?.field} = {snapshot?.current_stance}</code></p>
            <p>Agent 근거: {rationale}</p>
            <div className="actions">
                <button onClick={() => { /* prompt for new stance + reason */ }}>
                    의도 변경 확정
                </button>
                <button onClick={keepExisting}>기존 선호 유지</button>
                <button onClick={ignore} variant="ghost">무시 (이번만)</button>
            </div>
        </Modal>
    );
}
```

## Dependencies

depends_on: [02]

## Verification

- `cargo test --lib commands::agents_helpers::send_common::stance_check`:
  - `precheck` 반환값 3 가지 케이스:
    - 명확한 no-conflict (무관한 요청) → NoConflict
    - 명확한 conflict ("Rust 대신 Go 로 바꾸자") → Conflict
    - 애매 (키워드 매칭하지만 부정어 없음) → Ambiguous
  - INV-2 검증: no-conflict / conflict 케이스에서 model API 호출 mock 이 0 회 호출
  - Ambiguous 케이스에서만 model 호출 1 회
- `cargo test --lib commands::agents_helpers::send_common::prompt_assembly`:
  - stance_fragment 가 worldview 와 identity 사이에 삽입
- `npx vitest run src/lib/stanceConflictMarker.test.ts`:
  - extractStanceConflict — 정상 / malformed 케이스
  - stripStanceConflictMarker — 메시지에서 마커 제거
- `npx vitest run src/components/tunaflow/StanceConflictModal.test.tsx`:
  - "확정" 클릭 시 `record_user_preference_change` invoke
  - "무시" 클릭 시 invoke 호출 없음 (INV-5)
- 수동 E2E:
  1. Settings 에서 "engine_preference = CLI" 로 수동 stance 등록
  2. 새 대화에서 "SDK 로 해줘" 요청
  3. 첫 응답 전에 StanceConflictModal 표시 확인
  4. "기존 선호 유지" 선택 → agent 재질문 확인
  5. `SELECT COUNT(*) FROM preference_events WHERE ...` = 1 (원래 user 등록 하나만, ignore/keep 은 event 없음)

## Risks

- **Rule precheck false negative**: 한국어 부정어 체계가 복잡 — "~말고" / "~보다는" / "~대신" 외에도 다양. MVP 는 주요 패턴만 커버하고 ambiguous 로 fallback. 실측 로그 기반 개선.
- **Model verify 비용**: Haiku 호출이 매 ambiguous turn 마다 발화. 대화 빈도 높은 사용자는 누적 비용 체감 가능 — 본 plan 은 "rule 이 대다수 결정, ambiguous 는 드물다" 가정. 실측 후 ambiguous 기준 강화 필요 시 조정.
- **Timeout 시 ConflictSafe**: 안전 측 (false positive 증가). 사용자가 modal 을 자주 보면 피로도. Timeout 5s → 10s 조정 고려.
- **Marker 위치**: agent 가 응답 중간에 마커 넣으면 streaming 중 modal 뜨기 전에 텍스트 일부 렌더. streaming 완료 후 intercept 가 가장 깔끔 — 본 plan 이 이를 전제.
- **Modal race**: 같은 session 에서 여러 stance-conflict 가 연속 발화되면 modal queue 필요. MVP 는 한 번에 하나, 이전 modal open 중이면 신규 무시 (사용자 로그에 경고).
