> ⚠️ **SUPERSEDED / DEPRECATED (2026-04-23)** — 본 subtask 는 **설계 자체가 폐기됨**.
>
> 2026-04-23 사용자 리마인드: stance-conflict 판정은 rule + model verify + modal 엔진이 아니라 **LLM 자연 능력 + ContextPack 에 `worldview` / `identity_summary` 주입** 으로 충분. Modal 은 UX 침습 과다. agent 가 응답 안에서 자연스럽게 "현실적 관점" 제공하는 수준이 적합.
>
> 필요 시 `docs/ideas/stanceConflictStrongIdea.md` 로 idea 만 보존 가능. 본 파일은 git history 보존 목적으로 유지. Developer 는 `docs/archive/plans/superseded/` 로 git mv 고려.

# Subtask 03 — Stance-conflict detection (rule-first + small model verify + modal)

> 상위 plan: [userWorldviewInjectionPlan.md](./userWorldviewInjectionPlan.md)
> Codex round-1 / round-2 review 2026-04-23 반영 — partial-shift 감지 / Unknown fallback / composite key marker / hyphen-safe regex.

## Changed files

- `src-tauri/src/commands/agents_helpers/send_common/stance_check.rs` (신규) — rule precheck (partial-shift 감지 포함) + model verify + `Unknown` fallback.
- `src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs` — stance-check 결과를 compact fragment 로 주입.
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` — prepare 단계에서 stance_check 호출 + 결과 저장.
- `src/lib/stanceConflictMarker.ts` (신규) — `<!-- tunaflow:stance-conflict:... -->` 마커 파서 (composite key).
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
    let tokens = simple_tokenize(user_prompt);
    let mut matched: Vec<(PreferenceSnapshot, Vec<String>)> = Vec::new();

    for snap in snapshots {
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
            // Codex review 반영 — 부분 전환 silent pass 방지.
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
    // 부정어 휴리스틱 — "바꾸", "말고", "대신", "포기", "새로", "change", "instead" 등
    // MVP: false negative 허용, false positive 는 model verify 가 catch
}

/// Codex review — 부분 전환 패턴 감지.
/// "CLI 유지하면서 SDK 실험만" 같은 요청을 NoConflict 로 잘못 판정하지 않도록 Ambiguous escalate.
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

### 2. Model verify (ambiguous 케이스) — Unknown fallback

```rust
pub enum VerifyResult {
    Conflict { snapshot: PreferenceSnapshot, reason: String },
    NoConflict,
    Unknown { reason: String },   // timeout / model error
}

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
        Ok(Ok(resp)) => Ok(parse_verify_response(&resp.content)),
        Err(_) => Ok(VerifyResult::Unknown { reason: "timeout".into() }),
        Ok(Err(_)) => Ok(VerifyResult::Unknown { reason: "model_error".into() }),
    }
}
```

**Unknown 의 UI 처리**: modal 을 띄우지 않는다. 대신 응답 상단 inline warning:

> ⚠ 선호도 검증 미완료 (model {timeout|error}) — 진행하되 사후 검토 권장

사용자 "확인" 클릭 시 warning 닫힘. DB write 없음.

### 3. Compact fragment 주입

`prompt_assembly.rs`:

```rust
let stance_result = stance_check::run(&data.user_prompt, &snapshots, &recent_events, &app).await;
let stance_fragment = match stance_result {
    CheckResult::NoConflict => None,
    CheckResult::Conflict { snapshot, .. } =>
        Some(format!("⚠ Stance conflict detected: user previously stated `{}={}`. \
                      Current request may contradict. Ask for explicit confirmation.",
                     snapshot.field, snapshot.current_stance)),
    CheckResult::Unknown { reason } =>
        Some(format!("⚠ Preference check incomplete ({}). Proceed cautiously.", reason)),
};
if let Some(frag) = stance_fragment {
    sections.insert(after_worldview_idx, ("stance_check", frag));
}
```

### 4. Marker-based modal

Agent 응답에 `<!-- tunaflow:stance-conflict:<memory_name>:<field>:<rationale> -->` 마커 출현 시 UI 가 intercept. composite key 는 `preference_snapshots` 의 PK 와 정합. rationale 은 하이픈/개행 포함 허용 (non-greedy parsing).

```ts
// src/lib/stanceConflictMarker.ts
// Codex review 반영 — hyphen-safe non-greedy + composite key 파싱.

export function extractStanceConflict(text: string):
    { memoryName: string; field: string; rationale: string } | null {
    // 포맷: <!-- tunaflow:stance-conflict:<memory_name>:<field>:<rationale> -->
    // memory_name/field 는 ':' 금지 (task-02 sanitization). rationale 은 non-greedy 로 '-->' 직전까지.
    const m = text.match(/<!--\s*tunaflow:stance-conflict:([^:]+):([^:]+):([\s\S]*?)\s*-->/);
    return m ? { memoryName: m[1], field: m[2], rationale: m[3].trim() } : null;
}

export function stripStanceConflictMarker(text: string): string {
    return text.replace(/<!--\s*tunaflow:stance-conflict:[\s\S]*?-->/g, "");
}
```

Agent 메시지 렌더 경로:
- streaming 완료 시 `extractStanceConflict` 호출
- match 있으면 StanceConflictModal 표시 + 메시지 본문은 `stripStanceConflictMarker` 로 clean 렌더 (INV-6)

### 5. StanceConflictModal

```tsx
// Codex review 반영 — props 를 composite key 기반으로.
type Props = {
    memoryName: string;
    field: string;
    rationale: string;
    onClose: (result: { action: 'confirm_change' | 'keep_existing' | 'ignore' }) => void;
};

export function StanceConflictModal({ memoryName, field, rationale, onClose }: Props) {
    const [snapshot, setSnapshot] = useState<PreferenceSnapshot | null>(null);
    useEffect(() => {
        invoke<PreferenceSnapshot | null>('get_preference_snapshot', { memoryName, field })
            .then(setSnapshot);
    }, [memoryName, field]);

    const confirmChange = async (newStance: string, reason: string) => {
        await invoke('record_user_preference_change', {
            memoryName, field,
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
            <p>이전 선호: <code>{snapshot?.memory_name}.{snapshot?.field} = {snapshot?.current_stance}</code></p>
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
  - `precheck` 3 가지 케이스:
    - 명확한 no-conflict (무관한 요청) → NoConflict
    - 명확한 conflict ("Rust 대신 Go 로 바꾸자") → Conflict
    - 애매 (키워드 매칭하지만 부정어 없음, 혹은 partial-shift 키워드) → Ambiguous
  - `has_partial_shift_signal`:
    - "CLI 유지하면서 SDK 실험만" → true (single-match 에서 Ambiguous 강제)
    - "Rust 도 좋은데 Go 를 먼저 해보자" → true
    - "그냥 구현만 해줘" → false
  - INV-2 검증: no-conflict / conflict 결정적 케이스에서 model mock 0 회 호출
  - Ambiguous 케이스에서만 model 1 회 호출
  - `verify_ambiguous` timeout → `VerifyResult::Unknown { reason: "timeout" }`
- `cargo test --lib commands::agents_helpers::send_common::prompt_assembly`:
  - stance_fragment 가 worldview 와 identity 사이에 삽입
  - Unknown 상태도 fragment 주입 (소프트 경고)
- `npx vitest run src/lib/stanceConflictMarker.test.ts`:
  - 평범한 marker — `{memoryName, field, rationale}` 반환
  - rationale 에 하이픈 포함 (`sdk-first - cost risk`) → 전체 복원
  - multi-line rationale → 복원
  - 잘못된 포맷 → null
  - stripStanceConflictMarker — 모든 marker 제거
- `npx vitest run src/components/tunaflow/StanceConflictModal.test.tsx`:
  - mount 시 `get_preference_snapshot` invoke
  - "확정" 클릭 시 `record_user_preference_change` invoke
  - "무시" 클릭 시 invoke 호출 없음 (INV-5)
- 수동 E2E:
  1. Settings 에서 `engine_preference.cli_vs_sdk = CLI` 로 수동 등록
  2. 새 대화에서 "SDK 로 해줘"
  3. StanceConflictModal 표시 확인 — `engine_preference.cli_vs_sdk = CLI` 노출
  4. "기존 선호 유지" 선택 → agent 재질문 확인
  5. `SELECT COUNT(*) FROM preference_events` = 1 (원래 user 등록분만)

## Risks

- **Rule precheck false negative**: 한국어 부정어 체계 복잡. MVP 는 주요 패턴 + partial-shift 키워드만 커버. 실측 로그 기반 개선.
- **partial-shift 키워드 과다 매칭**: "먼저" 가 NoConflict 케이스에서도 자주 등장. false positive 로 model verify 호출 증가. 실측 후 ambiguous 분기 진입율 추적 필요.
- **Unknown 상태 처리**: UI 는 inline warning 만 — 사용자가 무시하고 진행 가능. 후속 PR 에서 "Unknown 이 연속 N 회면 Settings 이동 제안" 등의 degrade 경로 가능.
- **Marker 위치**: agent 가 응답 중간에 마커 넣으면 streaming 중 modal 뜨기 전에 텍스트 일부 렌더. streaming 완료 후 intercept 가 기본 — 본 plan 전제.
- **Modal race**: 같은 session 여러 conflict 연속 시 modal queue 필요. MVP 는 1개만, 이전 modal open 중이면 신규 무시.
- **composite key `:` 금지**: task-02 에 sanitization 규약 추가됨. 위반 시 marker 파싱 오류. 실수 방지용.
