# Subtask 05 — Settings UI (Rebuild 버튼 + 진행률 + 형태소 토글)

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src/components/settings/SearchSettings.tsx` — 신규. Settings 섹션 하나.
- `src/components/settings/SettingsPanel.tsx` (또는 동등) — `SearchSettings` import + 네비 entry 추가.
- `src/lib/api/search.ts` (신규) — `rebuildMessagesFts()` / `cancelRebuildMessagesFts()` wrapper.
- `src/types/events.ts` (필요 시 확장) — 이벤트 payload 타입.
- `src-tauri/src/commands/search/tokenizer.rs` — `morphological_query_enabled()` 를 **env var 우선 + `SEARCH_MORPH_FLAG: AtomicBool` fallback** 로 확장. DB 조회 없음.

## Change description

### 1. Backend — AtomicBool 런타임 flag (DB 저장 없음)

**Codex review 2026-04-22 (2차) 반영**: 초안은 "app_settings 있으면 사용, 없으면 env-only" 로 분기 규정하여 단일 소스 확정이 불분명했다. 실제 codebase 에 `app_settings` 테이블이 없으므로 DB 경로를 포기하고 **AtomicBool + FE localStorage** 로 단일화.

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static SEARCH_MORPH_FLAG: AtomicBool = AtomicBool::new(false);

pub fn morphological_query_enabled() -> bool {
    // env var 우선 (개발자 override / CI)
    if let Ok(v) = std::env::var("TUNAFLOW_MORPH_QUERY") {
        return matches!(v.trim().to_ascii_lowercase().as_str(), "1"|"true"|"on"|"yes");
    }
    // 런타임 flag (FE localStorage 에서 startup 에 주입됨)
    SEARCH_MORPH_FLAG.load(Ordering::Relaxed)
}
```

신규 commands (DB 접근 없음):

```rust
#[tauri::command]
pub fn set_morphological_query_enabled(enabled: bool) -> Result<(), AppError> {
    SEARCH_MORPH_FLAG.store(enabled, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_morphological_query_enabled() -> bool {
    SEARCH_MORPH_FLAG.load(Ordering::Relaxed)
}
```

Persist 는 FE localStorage 가 담당. 앱 재시작 시 FE startup hook 이 localStorage 값을 읽어 `invoke('set_morphological_query_enabled', { enabled })` 1회 호출해 AtomicBool 에 주입. 첫 렌더 전 이 동기화가 끝나지 않은 순간에는 flag OFF (기본값) — 사용자 체감 없음 (UI 검색창이 렌더되기 전 대개 완료).

### 2. Frontend — API wrapper

```typescript
// src/lib/api/search.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type RebuildProgress = { done: number; total: number };
export type RebuildComplete = { done: number; total: number; canceled: boolean };

export async function rebuildMessagesFts(
    onProgress: (p: RebuildProgress) => void,
    onComplete: (c: RebuildComplete) => void,
    onError: (err: string) => void,
): Promise<() => void> {
    const unlistenProgress = await listen<RebuildProgress>('messages_fts_rebuild_progress', (e) => onProgress(e.payload));
    const unlistenComplete = await listen<RebuildComplete>('messages_fts_rebuild_complete', (e) => onComplete(e.payload));
    const unlistenError = await listen<{ error: string }>('messages_fts_rebuild_error', (e) => onError(e.payload.error));
    // command invocation 은 fire-and-forget (await 하면 complete 이벤트 중복)
    invoke('rebuild_messages_fts').catch((e) => onError(String(e)));
    return () => { unlistenProgress(); unlistenComplete(); unlistenError(); };
}

export function cancelRebuildMessagesFts() {
    return invoke('cancel_rebuild_messages_fts');
}

export function setMorphEnabled(enabled: boolean) {
    return invoke('set_morphological_query_enabled', { enabled });
}

export function getMorphEnabled(): Promise<boolean> {
    return invoke('get_morphological_query_enabled');
}
```

### 3. 용량 추정 backend command

Rebuild 버튼 옆에 "예상 추가 용량" 을 표시하려면 백엔드가 3개 수를 반환해야 한다:

```rust
#[derive(serde::Serialize)]
pub struct RebuildEstimate {
    pub pending_rows: u64,            // content_tokenized IS NULL 인 message 수
    pub pending_content_bytes: u64,   // 해당 row 의 content 바이트 총합 (corpus-relative)
    pub est_added_bytes: u64,         // pending_content_bytes × 1.9 (rough estimate)
}

#[tauri::command]
pub fn estimate_messages_fts_rebuild(state: State<DbState>) -> Result<RebuildEstimate, AppError> {
    let r = state.read.lock().map_err(|_| AppError::Lock)?;
    // pending row 의 content 바이트 총합을 기준으로 추정.
    // overhead 모델 (plan Rationale §"비용/위험" 참조):
    //   content_tokenized (~0.7x) + fts_content (~0.7x) + fts_idx/data/docsize (~0.5x) ≈ 1.9x
    let (pending_rows, pending_bytes): (i64, i64) = r.query_row(
        "SELECT COUNT(*), COALESCE(SUM(length(content)), 0) FROM messages WHERE content_tokenized IS NULL",
        [], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    )?;
    let est_added_bytes = ((pending_bytes as f64) * 1.9) as u64;
    Ok(RebuildEstimate {
        pending_rows: pending_rows as u64,
        pending_content_bytes: pending_bytes as u64,
        est_added_bytes,
    })
}
```

> **Codex review 2026-04-22 반영**: 초안은 `db_size_bytes * 2` 로 전체 DB 파일 크기를 기준으로 했으나, messages corpus 이외 테이블 (conversations, plans, embeddings, trace_log …) 이 포함되어 부정확. `SUM(length(content))` 기반으로 교정. UI 라벨은 "rough estimate" 명시.

Settings 진입 시 1회 호출, 결과를 재구축 버튼 옆 hint 로 표시.

### 4. Frontend — SearchSettings.tsx

```tsx
// src/components/settings/SearchSettings.tsx
export function SearchSettings() {
    const [morph, setMorph] = useState(false);
    const [progress, setProgress] = useState<RebuildProgress | null>(null);
    const [running, setRunning] = useState(false);
    const [status, setStatus] = useState<'idle'|'running'|'done'|'canceled'|'error'>('idle');
    const [errorMsg, setErrorMsg] = useState<string>('');

    // startup 동기화: localStorage → backend AtomicBool 주입 + local state 반영.
    // (DB 는 사용하지 않음 — Q-4 resolution 참조.)
    useEffect(() => {
        const stored = localStorage.getItem('tunaflow.search.morphEnabled') === '1';
        setMorph(stored);
        setMorphEnabled(stored).catch((e) => console.warn('morph flag sync failed', e));
    }, []);

    const handleRebuild = async () => {
        setRunning(true);
        setStatus('running');
        setErrorMsg('');
        const cleanup = await rebuildMessagesFts(
            (p) => setProgress(p),
            (c) => { setStatus(c.canceled ? 'canceled' : 'done'); setRunning(false); cleanup(); },
            (err) => { setStatus('error'); setErrorMsg(err); setRunning(false); cleanup(); },
        );
    };

    const handleCancel = () => { cancelRebuildMessagesFts(); };

    const handleToggleMorph = async (next: boolean) => {
        // 단일 소스: localStorage (persist) + backend AtomicBool (런타임). 순서:
        //   1) localStorage 저장 (앱 재시작 대비)
        //   2) backend 반영 (현재 세션 검색 동작 즉시 변경)
        localStorage.setItem('tunaflow.search.morphEnabled', next ? '1' : '0');
        await setMorphEnabled(next);
        setMorph(next);
    };

    return (
        <section>
            <h3>검색 / Search</h3>

            <div className="setting-row">
                <label>한국어 형태소 검색 활성화</label>
                <Toggle checked={morph} onChange={handleToggleMorph} />
                <p className="hint">
                    조사/어미를 분리해 한국어 검색 재현율을 높입니다.
                    인덱스 재구축 후 활성화하세요.
                </p>
            </div>

            <div className="setting-row">
                <label>검색 인덱스</label>
                <button onClick={handleRebuild} disabled={running}>인덱스 재구축</button>
                {running && <button onClick={handleCancel}>취소</button>}
                {progress && (
                    <ProgressBar value={progress.done} max={progress.total}
                        label={`${progress.done} / ${progress.total}`} />
                )}
                {status === 'done' && <span className="status-ok">완료</span>}
                {status === 'canceled' && <span className="status-warn">취소됨</span>}
                {status === 'error' && <span className="status-error">{errorMsg}</span>}
                <p className="hint">
                    과거 메시지를 재인덱싱합니다. 대용량 프로젝트에서는 수 분이 걸릴 수 있습니다.
                </p>
            </div>
        </section>
    );
}
```

`ProgressBar` / `Toggle` 은 기존 컴포넌트 재사용 (search 해서 확인).

### 5. Settings navigation

`SettingsPanel` 의 section 목록에 `"검색"` 엔트리 추가. 순서는 Frontend 컨벤션 따름.

### 6. 헤더 검색창 "재구축 필요" 배너 (Developer review 로 추가)

Migration v45 적용 직후 `messages_fts` 가 비어있으므로 검색 결과가 0 건으로 나옴. 사용자가 이 원인을 모르면 "검색이 망가졌다" 고 오해 가능 — 다음 UX 를 **검색 결과 영역** 에 조건부 표시:

```tsx
// 결과가 0 이고 pending_rows > 0 일 때만 표시
<div className="search-rebuild-banner">
    검색 인덱스 재구축이 필요합니다 ({pendingRows.toLocaleString()} 개 메시지 대기 중)
    <a href="#" onClick={openSearchSettings}>Settings 에서 재구축</a>
</div>
```

판정 경로:
- 앱 startup 에 `estimate_messages_fts_rebuild` 1회 호출 → Zustand store (`searchIndexSlice.pendingRows`) 에 저장
- 검색 결과 컴포넌트가 `results.length === 0 && pendingRows > 0` 조건으로 배너 렌더
- 재구축 완료 이벤트 (`messages_fts_rebuild_complete`) 수신 시 pendingRows 0 으로 reset

**헤더 검색창 자체** 에 배너를 넣을지 (항상 표시) vs 결과 없음 상태에서만 표시할지는 Frontend UX 판단. 후자 권장 (노이즈 최소).

## Dependencies

depends_on: [03] — rebuild command + 이벤트. 04 (검색 경로 전환) 는 UX 완성도 측면에서 같이 있는 것이 이상적이지만 독립 머지 가능.

## Verification

- `npx vitest run src/components/settings/SearchSettings.test.tsx` — 신규 테스트:
  - 초기 상태에서 localStorage `tunaflow.search.morphEnabled = '1'` 시 토글 on + `invoke('set_morphological_query_enabled', { enabled: true })` 호출 1회.
  - "인덱스 재구축" 클릭 → `invoke('rebuild_messages_fts')` 호출.
  - `messages_fts_rebuild_progress` 이벤트 fire → progress bar 업데이트.
  - `messages_fts_rebuild_complete { canceled: false }` → 상태 "완료".
  - 취소 버튼 → `cancel_rebuild_messages_fts` 호출.
- `cargo test --lib commands::search::tokenizer::tests` — 신규: env var 우선 + AtomicBool fallback 동작. env 있음 → env 값 반환, env 없음 + AtomicBool=true → true 반환, 둘 다 없음 → false 반환.
- `npx tsc --noEmit` — exit 0.
- 수동 E2E: `npm run tauri dev` → Settings > 검색 → 재구축 실행 → 진행률 표시 확인 → 완료 후 토글 ON → 검색창에서 "플랜을" 쿼리 → plan 문서 hit.

## Risks

- **Tauri 이벤트 listener leak**: cleanup 반환값을 반드시 호출. 컴포넌트 unmount 시 cleanup 보장 (useEffect teardown).
- ~~**`app_settings` 테이블 없을 가능성**~~ — **확인됨**: `rg "app_settings\b" src-tauri/src` 결과 0건 (Codex review 2026-04-22 2차). 본 subtask 는 DB 경로를 포기하고 **FE localStorage → invoke('set_morphological_query_enabled')** 단일 소스로 확정.
- **검색 경로에 대한 런타임 감시**: Settings 에서 morph 토글 즉시 Backend 의 AtomicBool 갱신. 그러나 이미 열려 있는 검색 결과는 재쿼리 필요. UI 가 쿼리를 자동 재실행하진 않음 — 사용자가 재검색. 이는 기존 검색 UX 와 동일.
- **대용량 rebuild UX**: 진행률이 오래 걸리면 Settings 패널을 닫아도 job 은 백그라운드 지속. 이벤트 구독은 컴포넌트 unmount 시 해제되므로 "다시 Settings 를 열면 진행률이 안 보인다" 라는 UX 함정. **완화**: Settings 열 때 `get_messages_fts_rebuild_status` (별도 command; 본 subtask 범위 밖 — open question 으로 flag).
- **Toggle ON 상태에서 rebuild 를 하지 않은 경우**: 검색 결과가 비어 보임. Settings 에 "재구축이 필요합니다" 배너 추가 검토 — Q-4 와 함께 Developer 결정.
