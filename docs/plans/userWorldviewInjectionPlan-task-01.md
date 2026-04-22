# Subtask 01 — `user_worldview.md` 파일 + ContextPack 주입 + Settings 편집기

> 상위 plan: [userWorldviewInjectionPlan.md](./userWorldviewInjectionPlan.md)

## Changed files

- `src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs` — `worldview_fragment` 를 ContextPack `identity_fragment` 보다 앞에 삽입.
- `src-tauri/src/commands/worldview.rs` (신규) — 파일 read/write Tauri commands.
- `src-tauri/src/lib.rs` — 신규 command 등록.
- `src/components/settings/WorldviewSettings.tsx` (신규) — 텍스트 에디터 + "기본 문구 로드" 버튼.
- `src/components/settings/SettingsPanel.tsx` (또는 동등) — 네비에 Worldview 섹션 추가.

## Change description

### 1. 파일 경로 규칙

- Global: `~/.tunaflow/user_worldview.md`
- Project override: `<project_path>/.tunaflow/user_worldview.md` (있으면 global 을 무시)

Rust 측 helper:

```rust
// src-tauri/src/commands/worldview.rs
pub fn resolve_worldview_path(project_path: Option<&str>) -> Option<PathBuf> {
    if let Some(pp) = project_path {
        let project_p = PathBuf::from(pp).join(".tunaflow").join("user_worldview.md");
        if project_p.exists() { return Some(project_p); }
    }
    let home = dirs::home_dir()?;
    let global_p = home.join(".tunaflow").join("user_worldview.md");
    if global_p.exists() { Some(global_p) } else { None }
}

pub fn load_worldview(project_path: Option<&str>) -> Option<String> {
    let path = resolve_worldview_path(project_path)?;
    std::fs::read_to_string(path).ok()
}
```

### 2. ContextPack 주입

`src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs`:

```rust
// assemble_prompt() 내부
let worldview_fragment = worldview::load_worldview(data.project_path.as_deref())
    .map(|text| truncate_to_tokens(text, 500))     // 토큰 상한
    .filter(|t| !t.trim().is_empty());

// 실제 prompt_assembly 는 project/platform/agent-role 등을 identity 앞에 먼저 push.
// worldview 는 그들과 identity 사이 — 즉 identity 바로 앞에 들어간다 (Codex review 2026-04-23 반영).
let mut sections: Vec<(&str, String)> = Vec::new();
// ... project, platform, agent-role push (기존) ...
if let Some(wv) = worldview_fragment {
    sections.push(("worldview", wv));               // ★ identity 바로 앞
}
sections.push(("identity", identity_fragment));     // 기존 위치
// ... skills, recent_context, etc.
```

ContextPackMeta 의 `ctx_sections` (trace_log 용) 에도 `"worldview"` 포함해 관찰 가능.

### 3. 토큰 상한

500 tokens. 초과 시:
- 파일 write 는 허용 (경고만)
- ContextPack 주입 시 **앞 500 tokens 만 사용** (뒤 자름) + stderr 로 1회 경고
- Settings UI 에 `Used: N / 500 tokens` 카운터 + 500 초과 시 빨간 글씨

### 4. Settings UI

```tsx
export function WorldviewSettings() {
    const [content, setContent] = useState("");
    const [saved, setSaved] = useState(true);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        invoke<string | null>('get_worldview', { projectPath: null })
            .then((v) => { setContent(v ?? ""); setSaved(true); setLoading(false); });
    }, []);

    const save = async () => {
        await invoke('set_worldview', { content, projectPath: null });
        setSaved(true);
        toast.success("Worldview 저장됨 — 다음 요청부터 적용");
    };

    const loadDefault = () => {
        setContent(DEFAULT_WORLDVIEW_TEMPLATE);
        setSaved(false);
    };

    return (
        <section>
            <h3>User Worldview</h3>
            <p className="hint">
                에이전트가 매 요청 시 ContextPack 의 identity 바로 앞에서 참조하는 사용자 stance 문서입니다.
                최대 500 tokens.
            </p>
            <textarea
                value={content}
                onChange={(e) => { setContent(e.target.value); setSaved(false); }}
                className="w-full h-80 font-mono text-sm"
                placeholder="(비어있음 — 기본 문구 로드 또는 직접 작성)"
            />
            <TokenCounter current={countTokens(content)} max={500} />
            <div className="flex gap-2">
                <button onClick={save} disabled={saved}>저장</button>
                <button onClick={loadDefault} variant="ghost">기본 문구 로드</button>
            </div>
        </section>
    );
}
```

`DEFAULT_WORLDVIEW_TEMPLATE` (Open question Q-1 — 내용은 section 헤더만 두고 본문 비움):

```markdown
# User Worldview

## Ontology
(기본 세계관 — 직접 작성)

## Engagement preference
(agent 와의 협업 방식 선호 — 직접 작성)
```

사용자 철학을 tunaFlow 가 strong-suggest 하지 않도록 본문을 완전 비움. 헤더만 틀 제공.

### 5. 토글

Settings 에 체크박스:
- "Worldview 주입 활성화" (기본 ON)
- 끄면 `get_worldview` 는 `None` 반환 → 주입 스킵

`localStorage['tunaflow.worldview.enabled']` 로 persist + `invoke('set_worldview_enabled', { enabled })` 로 Rust AtomicBool 에 sync (설정 패널 스타일, `searchPipelineFromSecallPlan-part2-task-05.md` 의 패턴 재활용).

## Dependencies

depends_on: 없음.

## Verification

- `cargo test --lib commands::worldview`:
  - `resolve_worldview_path` — project override 우선, fallback global
  - `load_worldview` — 파일 없으면 None, 있으면 trimmed content
- `cargo test --lib commands::agents_helpers::send_common::prompt_assembly`:
  - Worldview fragment 주입 시 sections 에서 `"worldview"` 가 `"identity"` **바로 앞** 인덱스에 위치 (INV-1). project/platform/agent-role 등 identity 앞 섹션들이 worldview 앞에 유지됨도 함께 assert — Codex round-3 review 반영.
  - Worldview 비어있으면 sections 에 `"worldview"` 없음
- `npx vitest run src/components/settings/WorldviewSettings.test.tsx`:
  - 저장 후 버튼 disabled
  - "기본 문구 로드" 후 content=template
  - 500 초과 시 카운터 빨간색
- 수동 E2E:
  1. Settings > Worldview 열기 → "기본 문구 로드" → 직접 편집 → 저장
  2. 새 대화에서 임의 요청 → agent 응답 톤이 worldview 에 맞춰 변하는지 blind 관찰 (A/B 비교)
  3. `trace_log.ctx_sections` 에 `"worldview"` 포함 확인

## Risks

- **파일 쓰기 권한**: `~/.tunaflow/` 경로 생성 실패 (권한 등) 시 에러 toast. 대부분 OS 에서 문제 없음.
- **토큰 카운터 정확도**: `countTokens` 은 정확한 BPE 가 아니어도 대략 char/2 로 근사 가능. 정확도 민감도 낮음 (500 상한 자체가 여유).
- **Persona 와의 중복**: 사용자가 worldview 와 persona 에 같은 내용을 쓰면 중복 주입 — 토큰 낭비. 본 subtask 범위 밖 — 별도 deduplication plan 필요 시 후속.
- **Project override vs global**: 사용자가 글로벌에 stance 쓰고 프로젝트에 다른 stance 를 쓰면 후자 우선. 사용자가 "덮어쓴 줄 모르고 혼동" 할 수 있음. Settings UI 에 현재 적용 경로 표시 (`Used: <project>/.tunaflow/...` or `~/.tunaflow/...`) 권장.
- **Backward compat**: Worldview 가 없는 기존 사용자는 영향 없음. fragment 가 삽입 안 되므로 ContextPack 동작은 이전과 동일.
