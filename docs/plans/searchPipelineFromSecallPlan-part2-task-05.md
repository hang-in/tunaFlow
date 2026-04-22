# Subtask 05 — Settings UI (Rebuild 버튼 + 진행률 + 형태소 토글)

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src/components/settings/SearchSettings.tsx` — 신규. Settings 섹션 하나.
- `src/components/settings/SettingsPanel.tsx` (또는 동등) — `SearchSettings` import + 네비 entry 추가.
- `src/lib/api/search.ts` (신규) — `rebuildMessagesFts()` / `cancelRebuildMessagesFts()` wrapper.
- `src/types/events.ts` (필요 시 확장) — 이벤트 payload 타입.
- `src-tauri/src/commands/search/tokenizer.rs` — `morphological_query_enabled()` 를 env var 외에 **DB flag** 도 OR 체크하도록 확장. (Open question Q-4 를 "env var OR DB flag" 로 확정.)
- `src-tauri/src/db/migrations.rs` v45 안에 settings 테이블 항목 insert (없으면 skip — 기존 `app_settings` 같은 kv 테이블 재사용).

## Change description

### 1. Backend — DB flag 추가

`app_settings` kv 테이블이 있으면 `('search.morph_query_enabled', '0' | '1')` 로 저장. 없으면 신규 테이블은 도입하지 않고 **env var 방식만** 사용 (최소 스코프). 본 subtask 는 **기존 설정 저장소** 를 재사용.

`morphological_query_enabled()` 확장:

```rust
pub fn morphological_query_enabled() -> bool {
    // env var 우선 (개발자 override)
    if let Ok(v) = std::env::var("TUNAFLOW_MORPH_QUERY") {
        return matches!(v.trim().to_ascii_lowercase().as_str(), "1"|"true"|"on"|"yes");
    }
    // DB flag — 접근 비용 있으므로 OnceLock/RwLock 로 캐시.
    // 단순 구현: 매 호출마다 DB 조회 X. Tauri state 로 관리 + toggle command 에서 update.
    SEARCH_MORPH_FLAG.load(Ordering::Relaxed)
}
```

`SEARCH_MORPH_FLAG: AtomicBool` — 앱 startup 에 DB 에서 읽어 초기화. toggle command 가 set.

신규 commands:
```rust
#[tauri::command]
pub fn set_morphological_query_enabled(enabled: bool, state: State<DbState>) -> Result<(), AppError> {
    let w = state.write.lock().map_err(|_| AppError::Lock)?;
    w.execute(
        "INSERT INTO app_settings (key, value) VALUES ('search.morph_query_enabled', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![if enabled { "1" } else { "0" }]
    )?;
    SEARCH_MORPH_FLAG.store(enabled, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_morphological_query_enabled() -> bool {
    SEARCH_MORPH_FLAG.load(Ordering::Relaxed)
}
```

> **기존 `app_settings` 테이블 존재 여부 확인 필요** — 없으면 v45 migration 에 추가 (단 본 plan 범위 초과 여지. Developer 가 schema 확인 후 결정).

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

### 3. Frontend — SearchSettings.tsx

```tsx
// src/components/settings/SearchSettings.tsx
export function SearchSettings() {
    const [morph, setMorph] = useState(false);
    const [progress, setProgress] = useState<RebuildProgress | null>(null);
    const [running, setRunning] = useState(false);
    const [status, setStatus] = useState<'idle'|'running'|'done'|'canceled'|'error'>('idle');
    const [errorMsg, setErrorMsg] = useState<string>('');

    useEffect(() => { getMorphEnabled().then(setMorph); }, []);

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

### 4. Settings navigation

`SettingsPanel` 의 section 목록에 `"검색"` 엔트리 추가. 순서는 Frontend 컨벤션 따름.

## Dependencies

depends_on: [03] — rebuild command + 이벤트. 04 (검색 경로 전환) 는 UX 완성도 측면에서 같이 있는 것이 이상적이지만 독립 머지 가능.

## Verification

- `npx vitest run src/components/settings/SearchSettings.test.tsx` — 신규 테스트:
  - 초기 상태에서 `getMorphEnabled` mock 이 true 반환 시 토글 on.
  - "인덱스 재구축" 클릭 → `invoke('rebuild_messages_fts')` 호출.
  - `messages_fts_rebuild_progress` 이벤트 fire → progress bar 업데이트.
  - `messages_fts_rebuild_complete { canceled: false }` → 상태 "완료".
  - 취소 버튼 → `cancel_rebuild_messages_fts` 호출.
- `cargo test --lib commands::search::tokenizer::tests` — 신규: DB flag + env var OR 동작.
- `npx tsc --noEmit` — exit 0.
- 수동 E2E: `npm run tauri dev` → Settings > 검색 → 재구축 실행 → 진행률 표시 확인 → 완료 후 토글 ON → 검색창에서 "플랜을" 쿼리 → plan 문서 hit.

## Risks

- **Tauri 이벤트 listener leak**: cleanup 반환값을 반드시 호출. 컴포넌트 unmount 시 cleanup 보장 (useEffect teardown).
- **`app_settings` 테이블 없을 가능성**: 현재 tunaFlow 의 설정 저장 메커니즘 (Zustand + localStorage vs DB) 확인 필요. DB 가 아니면 Zustand persist + env var 없이 store 값만으로 `morphological_query_enabled()` 결정이 어렵다 — 이 경우 **localStorage → invoke('set_...') 동기화** 가 단일 소스.
- **검색 경로에 대한 런타임 감시**: Settings 에서 morph 토글 즉시 Backend 의 AtomicBool 갱신. 그러나 이미 열려 있는 검색 결과는 재쿼리 필요. UI 가 쿼리를 자동 재실행하진 않음 — 사용자가 재검색. 이는 기존 검색 UX 와 동일.
- **대용량 rebuild UX**: 진행률이 오래 걸리면 Settings 패널을 닫아도 job 은 백그라운드 지속. 이벤트 구독은 컴포넌트 unmount 시 해제되므로 "다시 Settings 를 열면 진행률이 안 보인다" 라는 UX 함정. **완화**: Settings 열 때 `get_messages_fts_rebuild_status` (별도 command; 본 subtask 범위 밖 — open question 으로 flag).
- **Toggle ON 상태에서 rebuild 를 하지 않은 경우**: 검색 결과가 비어 보임. Settings 에 "재구축이 필요합니다" 배너 추가 검토 — Q-4 와 함께 Developer 결정.
