# Trace 고도화 — abtop 패턴 적용

> Status: idea
> Created: 2026-04-04
> Updated: 2026-04-04 (코더 Opus 리뷰 반영)
> 출처: `_research/_util/abtop/` (Rust 4,950줄, "htop for AI agents")
> 기존 분석: `docs/ideas/abtopAnalysisForTunaFlow.md`

---

## 1. 현재 Trace vs abtop 갭 분석

### 현재 Trace가 하는 것

매 에이전트 호출마다 `trace_log`에 기록:
- 토큰 사용량 (input, output, cost_usd)
- 실행 시간 (duration_ms, status)
- ContextPack 메타 (mode, sections, length, truncated)
- OTel 호환 span (trace_id, span_id)

UI 표시:
- RuntimeStatusBar: 모드 뱃지 + 활성/스킵 섹션 수 + 누적 비용
- TracePanel: 스팬 히스토리 + 엔진별 집계 + 섹션 상세

### abtop이 추가로 하는 것

| 기능 | abtop 구현 | tunaFlow에 없는 것 |
|------|-----------|-------------------|
| 토큰 처리 속도 | 200-point 브레이유 스파크라인 | 시계열 시각화 없음 |
| 컨텍스트 윈도우 % | 모델별 % 바 + 80%/90% 경고 | 문자 수만 기록, % 미계산 |
| Rate limit | 5시간/7일 게이지 + 리셋 시간 | 추적 자체가 없음 |
| Orphan 프로세스 | PID 추적 + 포트 점유 감지 | rawq daemon만 관리 |
| Git 상태 | branch + added/modified 파일 수 | 표시 안 함 |
| 프로세스 리소스 | PID, RSS(MB), CPU% | 표시 안 함 |

---

## 2. 고도화 항목

### 2.1 [P0] 토큰 처리 속도 시각화

**현재**: `trace_log`에 `output_tokens`과 `duration_ms`가 있지만 속도 계산/시각화 없음.

**추가**:

```typescript
// TracePanel에서 계산 (DB 변경 없음)
const tokensPerSec = span.outputTokens / (span.durationMs / 1000);

// 시계열 데이터: 대화의 모든 span에서 tok/s 추출
const speedHistory = traces.map(t => ({
  timestamp: t.recordedAt,
  tokPerSec: t.outputTokens / (t.durationMs / 1000),
  engine: t.engine,
}));
```

**시각화**: 미니 라인 차트 또는 바 차트 (React 컴포넌트)

```
Token speed (tok/s)
 80 ┤                    ╭─╮
 60 ┤          ╭─╮  ╭─╮ │ │
 40 ┤    ╭─╮  │ │  │ │ │ │ ╭─╮
 20 ┤╭─╮ │ │  │ │  │ │ │ │ │ │
  0 ┤╰─╯─╰─╯──╰─╯──╰─╯─╰─╯─╰─╯
     t1  t2   t3   t4   t5  t6
```

**가치**: "이번 대화에서 토큰 속도가 점점 느려지네" → 컨텍스트가 커지고 있다는 신호. 모델 변경이나 대화 분리 타이밍 판단 근거.

**구현**:
- TracePanel에 speed 계산 로직 추가 (~20줄)
- 미니 차트 컴포넌트 (SVG 또는 canvas, ~80줄)
- DB 변경 없음

### 2.2 [P0] 컨텍스트 윈도우 사용률 (%)

**현재**: `context_length`(문자 수)만 기록. 모델 한도 대비 % 미표시.

**추가**:

```typescript
// 모델별 최대 컨텍스트 (토큰 기준)
const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  "claude-sonnet-4-6": 200_000,
  "claude-opus-4-6": 1_000_000,
  "claude-haiku-4-5": 200_000,
  "gpt-4o": 128_000,
  "gemini-2.5-pro": 1_000_000,
  // ... engineModelSlice에서 동적 로딩도 가능
};

// 사용률 계산 — input_tokens를 직접 사용 (trace_log에 이미 기록됨)
// ※ context_length ÷ 4 추정은 영어 기준. 한국어는 토큰당 1-2자로 2-3배 오차 발생.
//    input_tokens가 실제 API 응답값이므로 이것을 사용.
const maxTokens = MODEL_CONTEXT_LIMITS[span.model] ?? 200_000;
const usagePercent = (span.inputTokens / maxTokens) * 100;
```

**시각화**: 프로그레스 바 + 색상 경고

```
Context: ████████████░░░░░░░░ 62%  (claude-sonnet-4-6, 200K)
         ████████████████████ 91%  ⚠️ (gpt-4o, 128K)
```

색상 규칙 (abtop 패턴):
- 0-60%: 초록
- 60-80%: 노랑
- 80-90%: 주황
- 90%+: 빨강 + 경고 아이콘

**표시 위치**:
- RuntimeStatusBar: 컨텍스트 모드 뱃지 옆에 % 표시
- TracePanel: 각 스팬에 프로그레스 바

**구현**:
- 모델별 한도 테이블 (engineModelSlice 확장 또는 상수, ~30줄)
- 프로그레스 바 컴포넌트 (~40줄)
- RuntimeStatusBar에 % 표시 추가 (~10줄)
- DB 변경 없음

### 2.3 [P1] Rate Limit 추적

> P0 → P1 하향 (코더 Opus 리뷰): `~/.claude/abtop-rate-limits.json`은 abtop의 StatusLine hook이 생성. abtop 미설치 사용자(대다수)에게는 동작하지 않는 기능. SDK 전환 후 API 응답 헤더에서 직접 파싱하는 것이 보편적 대안.

**현재**: 추적 자체가 없음. 에이전트가 느려져도 원인 파악 불가.

**추가**:

abtop이 읽는 파일:
- Claude: `~/.claude/abtop-rate-limits.json` (StatusLine hook으로 생성)
- 형식:
```json
{
  "five_hour_pct": 75.0,
  "five_hour_resets_at": 1712345678,
  "seven_day_pct": 30.0,
  "seven_day_resets_at": 1712999999,
  "updated_at": 1712340000
}
```

**tunaFlow에서의 구현**:

```rust
// src-tauri/src/commands/diagnostics.rs (새 파일)

#[tauri::command]
pub fn get_rate_limit_info() -> Result<Option<RateLimitInfo>, AppError> {
    let path = dirs::home_dir()
        .map(|h| h.join(".claude/abtop-rate-limits.json"));

    match path {
        Some(p) if p.exists() => {
            let content = std::fs::read_to_string(&p)?;
            let info: RateLimitInfo = serde_json::from_str(&content)?;
            // 5분 이상 오래된 데이터는 무시
            if info.updated_at + 300 < now_epoch_secs() {
                return Ok(None);
            }
            Ok(Some(info))
        }
        _ => Ok(None),
    }
}

pub struct RateLimitInfo {
    pub five_hour_pct: Option<f64>,
    pub five_hour_resets_at: Option<u64>,
    pub seven_day_pct: Option<f64>,
    pub seven_day_resets_at: Option<u64>,
    pub updated_at: Option<u64>,
}
```

**시각화**: RuntimeStatusBar에 게이지 표시

```
Rate: ████████████░░░░ 75% (5h, resets 14:30)
```

**주의**: 이 파일은 Claude Code의 StatusLine hook이 생성합니다. tunaFlow 사용자가 abtop을 설치하지 않았으면 파일이 없을 수 있음 → `Option<RateLimitInfo>` 반환, 없으면 UI에서 숨김.

**대안**: tunaFlow가 직접 Claude API 응답 헤더에서 rate limit 정보를 추출. 하지만 현재 CLI subprocess 방식이라 헤더 접근 불가. SDK 전환 후에 가능.

**구현**:
- 새 Tauri command 1개 (~40줄)
- RuntimeStatusBar에 조건부 게이지 (~30줄)
- TracePanel에 rate limit 섹션 (~50줄)
- DB 변경 없음

---

## 3. 중간 우선순위 항목

### 3.1 [P2] Orphan 프로세스 감지

> P1 → P2 하향 (코더 Opus 리뷰): 실질적 필요성 낮음. `ps`/`lsof`는 macOS/Linux 전용이며 Windows 지원 시 `sysinfo` 크레이트(새 의존성) 필요.

**현재**: rawq daemon만 관리. 에이전트가 남긴 다른 프로세스는 추적 안 함.

**abtop 패턴**:
1. 에이전트 실행 시 자식 프로세스 PID 기록
2. 에이전트 완료 후 자식이 살아있고 포트를 점유하면 orphan으로 표시
3. 사용자에게 "이 프로세스가 포트 3000을 점유 중" 경고

**tunaFlow 적용**:

```rust
// 에이전트 완료 시 체크
pub fn check_orphan_processes(project_path: &str) -> Vec<OrphanProcess> {
    // ps -eo pid,ppid,rss,command | grep project_path
    // lsof -i -P | grep LISTEN | match PIDs
}

pub struct OrphanProcess {
    pub pid: u32,
    pub command: String,
    pub port: Option<u16>,
    pub rss_mb: u64,
}
```

**표시**: TracePanel 하단에 "Orphan processes" 경고 섹션.

**구현**:
- 새 Tauri command (~60줄)
- TracePanel에 경고 섹션 (~40줄)
- DB 변경 없음

### 3.2 [P1] Git 상태 표시 + dirty 감지

> 추가 레퍼런스: `_research/_util/claude-status-bar/` — `git diff --quiet HEAD`로 dirty 감지

**현재**: 프로젝트 경로는 알지만 git 상태를 표시 안 함.

**추가**:

```rust
#[tauri::command]
pub fn get_git_status(project_path: String) -> Result<GitStatus, AppError> {
    // git -C {project_path} branch --show-current
    // git -C {project_path} diff --quiet HEAD → dirty 여부
    // git -C {project_path} status --porcelain | wc -l
    Ok(GitStatus {
        branch: "main".into(),
        dirty: true,             // 미커밋 변경 있음
        added: 3,
        modified: 7,
        untracked: 2,
    })
}
```

**표시**: RuntimeStatusBar 또는 커스텀 타이틀바에 branch + dirty + 변경 파일 수.

```
🌿 main* (+3 ~7)     ← *는 dirty 표시
🌿 main (+0 ~0)      ← clean 상태
```

**가치**: Developer 에이전트 실행 전후의 변경량 비교. "이 에이전트가 파일 7개를 수정했다" 확인.

**구현**:
- 새 Tauri command (~40줄, dirty 체크 포함)
- RuntimeStatusBar에 조건부 표시 (~20줄)
- DB 변경 없음

### 3.3 [P1] 시간당 비용 ($/h)

> 추가 레퍼런스: `_research/_util/claude-status-bar/` — `cost / (duration_ms / 1000) * 3600`

**현재**: RuntimeStatusBar에 누적 비용($X.XX)은 있지만, **시간당 비용**은 없음.

**추가**:

```typescript
// RuntimeStatusBar.tsx
const sessionDurationHours = (Date.now() - sessionStartTime) / 3600000;
const hourlyRate = sessionDurationHours > 0 ? totalCost / sessionDurationHours : 0;

// 표시
`💰 $${totalCost.toFixed(2)} ($${hourlyRate.toFixed(2)}/h)`
```

**가치**: "이 세션이 비싼지 싼지"를 직관적으로 파악. 시간당 $2 vs $0.10은 모델/모드 선택에 영향.

**구현**:
- RuntimeStatusBar에 계산 + 표시 (~10줄 FE)
- 세션 시작 시간은 첫 trace_log의 recorded_at 사용
- DB 변경 없음

---

## 4. 구현 계획

### Phase 1: 프론트엔드만 (DB 변경 없음)

```
4-1. 토큰 속도 계산 + 미니 차트
     - TracePanel에 speed 계산 (~20줄)
     - SVG 미니 차트 컴포넌트 (~80줄)

4-2. 컨텍스트 윈도우 % 표시
     - 모델별 한도 테이블 (~30줄)
     - 프로그레스 바 컴포넌트 (~40줄)
     - RuntimeStatusBar에 % 추가 (~10줄)
```

### Phase 2: 백엔드 커맨드 추가 (DB 변경 없음)

```
4-3. Git 상태 표시
     - get_git_status() Tauri command (~30줄)
     - 기존 git 관련 기능(linkGitBranch 등) 재사용 가능한지 먼저 확인
     - RuntimeStatusBar 표시 (~20줄)

4-4. Rate limit 추적 (abtop 설치 사용자 또는 SDK 전환 후)
     - get_rate_limit_info() Tauri command (~40줄)
     - RuntimeStatusBar 게이지 (~30줄)
     - TracePanel 섹션 (~50줄)
     - abtop 미설치 시 graceful skip (UI에서 숨김)
```

### Phase 3: 진단 기능 (선택, macOS/Linux only)

```
4-5. Orphan 프로세스 감지
     - check_orphan_processes() command (~60줄)
     - TracePanel 경고 섹션 (~40줄)
     - macOS/Linux: ps + lsof
     - Windows: sysinfo 크레이트 필요 (새 의존성) 또는 미지원
```

---

## 5. 변경 범위 예측

| Phase | 신규 코드 | 수정 코드 | DB 변경 | 파일 |
|-------|----------|----------|---------|------|
| Phase 1 | ~170줄 | ~30줄 | 없음 | TracePanel.tsx, RuntimeStatusBar.tsx, 새 차트 컴포넌트 |
| Phase 2 | ~170줄 | ~50줄 | 없음 | 새 diagnostics.rs, TracePanel.tsx, RuntimeStatusBar.tsx, lib.rs |
| Phase 3 | ~100줄 | ~20줄 | 없음 | diagnostics.rs 확장, TracePanel.tsx |

**총 예상**: ~440줄 신규 + ~100줄 수정. DB 변경 없음, 기존 API 변경 없음.

---

## 6. abtop에서 채택하지 않을 것

| 기능 | 이유 |
|------|------|
| 프로세스 CPU/RSS 모니터링 | 에이전트 subprocess 수명이 짧아 모니터링 가치 낮음 |
| Subagent 추적 | tunaFlow는 Branch 기반 분리. abtop의 subagent 모델과 구조 다름 |
| 브레이유 스파크라인 (TUI) | React UI에서 SVG/Canvas 차트가 더 적합 |
| 전체 collector 아키텍처 | tunaFlow는 내부 상태를 이미 알고 있음. 외부 탐색 불필요 |
| Summary 생성 (claude --print) | tunaFlow는 compressed_memory로 이미 요약 |

---

## 7. 아키텍처 패턴 참고

### SharedProcessData (abtop 핵심 패턴)

```rust
// abtop: 한 틱에 한 번만 ps/lsof 실행, 모든 collector가 공유
pub struct SharedProcessData {
    pub process_info: HashMap<u32, ProcInfo>,
    pub children_map: HashMap<u32, Vec<u32>>,
    pub ports: HashMap<u32, Vec<u16>>,
}
```

tunaFlow에 적용 시: `get_runtime_diagnostics()` 단일 커맨드로 rate limit + git status + orphan을 한 번에 수집. 개별 커맨드 3번 호출보다 효율적.

```rust
#[tauri::command]
pub fn get_runtime_diagnostics(project_path: String) -> Result<RuntimeDiagnostics, AppError> {
    Ok(RuntimeDiagnostics {
        rate_limit: get_rate_limit_info_internal()?,
        git_status: get_git_status_internal(&project_path)?,
        orphan_processes: check_orphan_processes_internal(&project_path)?,
    })
}
```

### 폴링 전략 (abtop 패턴)

```
빠른 폴링 (2초): 에이전트 상태, 토큰 업데이트
느린 폴링 (10초): ps, lsof, git status, rate limit
```

tunaFlow에서: RuntimeStatusBar의 `list_active_jobs()` 폴링(현재 2초)에 rate limit 체크를 함께 하되, git/orphan은 10초 간격으로 분리.

---

## 참고

- claude-status-bar 소스: `_research/_util/claude-status-bar/` (Bash 97줄)
  - 시간당 비용 계산: `statusline.sh` (cost/duration*3600)
  - Context % 3-tier 경고: 🧊(<70%) ⚠️(70-79%) ❗(≥80%)
  - Git dirty 감지: `git diff --quiet HEAD`
  - Rate limit: Claude Code statusline JSON stdin에서 추출
- abtop 소스: `_research/_util/abtop/` (Rust 4,950줄)
- abtop 세션 모델: `_research/_util/abtop/src/model/session.rs` (174줄)
- abtop 프로세스 스캔: `_research/_util/abtop/src/collector/process.rs` (142줄)
- abtop orphan 감지: `_research/_util/abtop/src/collector/mod.rs` (148-193줄)
- abtop rate limit: `_research/_util/abtop/src/collector/rate_limit.rs` (128줄)
- 기존 abtop 분석: `docs/ideas/abtopAnalysisForTunaFlow.md`
- tunaFlow Trace 백엔드: `src-tauri/src/commands/agents_helpers/trace_log.rs`
- tunaFlow TracePanel: `src/components/tunaflow/context-panel/TracePanel.tsx` (483줄)
- tunaFlow RuntimeStatusBar: `src/components/tunaflow/RuntimeStatusBar.tsx`
