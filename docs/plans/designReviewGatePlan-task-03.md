# Subtask 03 — Plan adopt (문서 머지) + Transcript auto-append + Settings UI

> 상위 plan: [designReviewGatePlan.md](./designReviewGatePlan.md)

## Changed files

- `src-tauri/src/commands/design_review.rs` — `adopt_design_review` Tauri command 신규. transcript append 유틸 포함.
- `src-tauri/src/commands/branches.rs` — 기존 `adopt_branch` 는 유지. design_review 는 별도 경로.
- `src/components/tunaflow/chat/PlanProposalCard.tsx` — verdict=pass 또는 force-approved 시 "승인 완료" 상태 + "구현 시작" 버튼 노출.
- `src/components/settings/DesignReviewSettings.tsx` (신규) — reviewer engine 선택 (codex | gemini) + 기본 engine 기록.
- `src/lib/api/design_review.ts` (신규) — Tauri command wrapper + event listener helpers.

## Change description

### 1. Plan document adopt

```rust
// src-tauri/src/commands/design_review.rs
#[tauri::command]
pub async fn adopt_design_review(
    branch_id: String,
    plan_document_path: String,
    main_conversation_id: String,
    state: State<'_, DbState>,
    app: AppHandle,
) -> Result<(), AppError> {
    // 1) transcript auto-append (§2)
    append_review_transcript(&plan_document_path, &branch_id, &state)?;

    // 2) frontmatter 에 review 메타 주입 (idempotent)
    inject_review_frontmatter(&plan_document_path, &branch_id, &state)?;

    // 3) main conversation 에 시스템 메시지
    let (plan_title, rounds, engine) = read_review_meta(&branch_id, &state)?;
    let sys_msg = format!(
        "Plan [{}]({}) approved after {} review round(s) · reviewer={}",
        plan_title, plan_document_path, rounds, engine
    );
    insert_system_message(&state, &main_conversation_id, &sys_msg)?;

    // 4) branch archived=1
    {
        let w = state.write.lock().map_err(|_| AppError::Lock)?;
        w.execute("UPDATE branches SET archived=1 WHERE id=?1", [&branch_id])?;
    }

    app.emit("design_review_adopted", serde_json::json!({
        "branchId": branch_id,
        "planDocumentPath": plan_document_path,
        "rounds": rounds,
    }))?;
    Ok(())
}
```

### 2. Transcript auto-append

```rust
fn append_review_transcript(
    plan_path: &str,
    branch_id: &str,
    state: &DbState,
) -> Result<(), AppError> {
    let rounds = collect_review_rounds(branch_id, state)?;  // shadow conv 의 reviewer 메시지들
    let today = chrono::Local::now().format("%Y-%m-%d");

    let mut appended = String::from("\n\n---\n");
    for (round_idx, round_data) in rounds.iter().enumerate() {
        appended.push_str(&format!(
            "\n## Codex Review (Round {} — {})\n\n<details>\n<summary>verdict={} · BLOCKER {} · MAJOR {} · MINOR {}</summary>\n\n{}\n\n</details>\n",
            round_idx + 1,
            today,
            round_data.verdict,
            round_data.blocker_count,
            round_data.major_count,
            round_data.minor_count,
            round_data.full_markdown.trim(),
        ));
    }

    let existing = std::fs::read_to_string(plan_path)
        .map_err(|e| AppError::Agent(format!("read plan: {e}")))?;
    let merged = format!("{}\n{}\n", existing.trim_end(), appended.trim());
    std::fs::write(plan_path, merged)
        .map_err(|e| AppError::Agent(format!("write plan: {e}")))?;
    Ok(())
}
```

`<details>` collapsible 로 plan 본문 가독성 유지.

### 3. Frontmatter 주입

```rust
fn inject_review_frontmatter(
    plan_path: &str,
    branch_id: &str,
    state: &DbState,
) -> Result<(), AppError> {
    // YAML frontmatter 파싱 → design_reviewed_at / review_rounds / reviewer_engine 필드 삽입 또는 갱신
    // 주의: 기존 frontmatter 보존 (title, status, priority 등 변경 금지)
    // 라이브러리: serde_yaml 또는 간단한 regex 기반 (frontmatter 는 문서 상단 `---` 블록)
}
```

frontmatter 스키마 추가:
```yaml
design_reviewed_at: 2026-04-22T19:34:00+09:00
review_rounds: 3
reviewer_engine: codex
force_approved_at_round: null   # or 3 if force approve
blocker_findings: []            # force approve 시에만 채워짐
```

### 4. Settings UI — reviewer engine

`src/components/settings/DesignReviewSettings.tsx`:

```tsx
export function DesignReviewSettings() {
  const [engine, setEngine] = useState<'codex' | 'gemini'>('codex');

  useEffect(() => {
    const stored = localStorage.getItem('tunaflow.designReview.engine') ?? 'codex';
    setEngine(stored as any);
  }, []);

  const handleChange = (next: 'codex' | 'gemini') => {
    localStorage.setItem('tunaflow.designReview.engine', next);
    setEngine(next);
  };

  return (
    <section>
      <h3>Design Review</h3>
      <div className="setting-row">
        <label>Reviewer 엔진</label>
        <Select value={engine} onChange={handleChange}>
          <option value="codex">Codex (기본, Architect=Opus 와 blind)</option>
          <option value="gemini">Gemini</option>
        </Select>
        <p className="hint">
          Architect 가 제안한 plan 을 다른 vendor 모델이 독립 검증합니다.
          Codex CLI 가 설치돼 있어야 합니다.
        </p>
      </div>
    </section>
  );
}
```

engine 값은 `open_design_review_branch` 호출 시 argument 로 전달.

### 5. PlanProposalCard 상태 전이

```tsx
// Zustand 에서 rtVerdict, rtRound 읽어옴
if (approvalPath === 'rt' && rtVerdict === 'pass') {
  return (
    <div>
      <SuccessBadge>RT 검토 완료 ({rtRound}라운드) · verdict=pass</SuccessBadge>
      <Button onClick={onStartImplementation}>구현 시작</Button>
    </div>
  );
}
if (rtVerdict === 'escalate_to_human' || (rtRound >= 3 && rtVerdict === 'fail')) {
  return (
    <div>
      <WarningBadge>3라운드 후에도 blocker 있음 — 사용자 확인 필요</WarningBadge>
      <Button variant="ghost" onClick={openForceApprovalModal}>강제 승인</Button>
      <Button variant="ghost" onClick={onReworkPlan}>plan 재작성 요청</Button>
      <Button variant="ghost" onClick={onDiscardPlan}>plan 폐기</Button>
    </div>
  );
}
```

"구현 시작" 은 `adopt_design_review` invoke → 성공 시 Dev 단계 (기존 subtask 생성 flow) 진입.

## Dependencies

depends_on: [02]

## Verification

- `cargo test --lib commands::design_review::adopt_design_review`:
  - 정상 adopt 후 plan 문서에 `## Codex Review (Round N)` append 확인
  - frontmatter 에 `design_reviewed_at`, `review_rounds`, `reviewer_engine` 주입 확인
  - main conversation 에 시스템 메시지 1건 추가 확인
  - branch archived=1 확인
- `cargo test --lib commands::design_review::append_review_transcript`:
  - 여러 라운드 append 시 순서 보존
  - 기존 plan 본문 변경 없음 (말미 append 만)
  - `<details>` 블록 문법 정합
- `npx vitest run src/components/settings/DesignReviewSettings.test.tsx`:
  - 기본 engine=codex
  - Select 변경 시 localStorage 저장
- 수동 E2E:
  1. plan proposal → "RT 검토 먼저" 클릭
  2. 1~3 라운드 진행 (mock 또는 실제 Codex)
  3. "구현 시작" 클릭 → plan 문서 말미에 reviewer transcript 추가됨 확인
  4. frontmatter 에 메타 주입 확인
  5. main 대화에 "Plan approved" 시스템 메시지 추가 확인

## Risks

- **Plan 파일 쓰기 권한**: tunaFlow 프로세스가 `docs/plans/` 에 쓰기 가능해야 함. macOS/Linux 는 대개 OK, Windows 일부 환경 (OneDrive 동기화 등) 에서 제약 가능. 실패 시 에러 toast + adopt 롤백 (branch archived 취소).
- **Git 영향**: plan 파일이 git-tracked 라 transcript append 가 working tree 더러움. 사용자가 의도 못 한 커밋에 포함될 위험. 대응: adopt 후 "plan 문서 업데이트됨 — git diff 확인" toast. 또는 설정으로 "transcript append OFF" 옵션.
- **Frontmatter 파싱 깨짐**: 일부 plan 은 frontmatter 없이 작성됐을 수 있음. adopt 시 frontmatter 없으면 상단에 신규 생성 (`---` block 추가). regex 기반 단순 파싱 권장 — serde_yaml 은 heavy.
- **transcript 길이**: 3 라운드 × 수 KB reviewer 응답 → plan 문서가 2~3배 커짐. `<details>` collapsible 로 가독성 유지. 매우 큰 plan (10+ 라운드는 INV-3 으로 막힘) 은 애초 발생 안 함.
- **Race condition**: 사용자가 adopt 클릭 직후 RT 가 백그라운드에서 추가 라운드 실행 가능. adopt 시점에 `UPDATE branches SET archived=1` 후에는 신규 라운드 command 가 "branch archived" 에러로 차단되어야 함. INV-3 과 함께 방어.
- **engine=gemini 실제 검증**: Codex 외 Gemini 로 돌려본 적 없음. guidance 수용성 검증 필요. 초기 릴리스는 Codex 만, Gemini 는 후속 PR 에서 enable — 본 subtask 는 UI 드롭다운만 준비.
