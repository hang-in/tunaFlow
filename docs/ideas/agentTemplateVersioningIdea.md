# 에이전트 템플릿 버전관리 — drift 허용 구조로 전환

> Status: idea
> Created: 2026-04-22
> Trigger: `ensure_workflow_templates` 가 `docs/agents/*.md` 를 무조건 덮어쓰는 버그(PR #117 로 즉시 수정) + 사용자 질문 "에이전트가 참고하는 문서도 버전관리 개념 도입하면 좋겠다"
> 관련: `src-tauri/src/commands/project_tools.rs` (templates), `docs/reference/documentVersioningPolicy_2026-03-30.md`, `scripts/publish-skills.sh` (skills snapshot 모델)

---

## 1. 현재 상태

| 문서 영역 | 버전관리 상태 | 평가 |
|---|---|---|
| Skills (`~/.tunaflow/skills/`) | `_snapshot.json` + `publish-skills.sh` | ✅ 잘 됨. snapshot 방식, 공급자별 격리 |
| `docs/agents/{architect,developer,reviewer}.md` | Rust 상수 + **무조건 덮어쓰기** | ⚠️ 드리프트 은닉 — 프로젝트별 로컬 수정이 매번 사라짐 |
| `CLAUDE.md` | git-tracked 일반 파일 | ⚠️ "버전" 개념 없음 |
| `AGENTS.md` / `GEMINI.md` | `conventions_sync.rs` 로 프로젝트별 sync | △ sync 메커니즘만, 버전 메타 없음 |
| `docs/reference/documentVersioningPolicy_2026-03-30.md` | 정책 문서 존재 | 📝 정책 있음, 적용 일관성은 부분적 |

---

## 2. 결론

- **Skills snapshot 모델이 성공적** → 이 패턴을 에이전트 템플릿에도 확장.
- 현재 "상수 하드코딩 + 무조건 덮어쓰기" 는 반(反) 버전관리. **tunaFlow 자체가 tunaflow 프로젝트** 이라 자기 자신의 파일을 구버전으로 회귀시키는 재귀 버그 발생 (PR #117 참고).
- 해결 방향: **semver 기반 drift 허용** + **프로젝트 로컬 수정 보존**.

---

## 3. 제안 구조

### 3.1 디렉터리

```
docs/agents/
├── architect.md           ← git tracked, 수정 가능. 실제 로딩 대상
├── developer.md
├── reviewer.md
└── _meta/
    └── agents-snapshot.json
```

### 3.2 `_meta/agents-snapshot.json` 스키마

```json
{
  "schema_version": 1,
  "min_app_version": "0.2.0",
  "templates": {
    "architect.md": {
      "version": "2026-04-22-3",
      "sha256": "ab34...",
      "description": "slug canonical + tiered message inspection 포함"
    },
    "developer.md": {
      "version": "2026-04-22-2",
      "sha256": "ef56...",
      "description": "tiered message inspection 포함"
    },
    "reviewer.md": {
      "version": "2026-04-20-1",
      "sha256": "cd78...",
      "description": "3-point checklist + structured verdict"
    }
  }
}
```

### 3.3 덮어쓰기 정책

`ensure_workflow_templates` 재설계:

1. 프로젝트 디렉터리에 `_meta/agents-snapshot.json` 읽기 시도
2. tunaFlow 앱 bundle 에 내장된 최신 snapshot 과 비교
3. 각 파일 별로:
   - **없으면**: 최신 버전으로 생성 (+ snapshot 갱신)
   - **있고 snapshot 버전 = 최신**: 그대로 둠
   - **있고 snapshot 버전 < 최신 + 파일 sha256 = snapshot.sha256** (사용자 로컬 수정 없음): 자동 upgrade (+ snapshot 갱신)
   - **있고 snapshot 버전 < 최신 + 파일 sha256 ≠ snapshot.sha256** (로컬 수정 있음): **덮어쓰지 않음**. UI 에 "업그레이드 가능, 로컬 수정 보존" 배지 표시. 사용자가 수동 선택.
4. snapshot 없는 기존 프로젝트: "처음 선택" 으로 간주. 사용자에게 "최신 템플릿으로 동기화" 프롬프트 (opt-in).

### 3.4 Rust 내장 vs 외부 파일

현재: Rust `const ARCHITECT_TEMPLATE` 상수. 컴파일 시 고정.

옵션 A) **상수 유지 + snapshot 메타 병행** — 가장 단순. 상수 자체가 single source of truth 역할.
옵션 B) **앱 번들 리소스에서 로딩** (`include_str!("../../docs/agents/architect.md")`) — 빌드 시 git 파일이 자동 반영. 드리프트 불가.
옵션 C) **런타임 fetch** — 사용자가 최신 템플릿을 별도 선택 다운로드. 오버엔지니어링.

**권장: B**. `include_str!` 매크로로 빌드 시 git 파일을 강제 포함 → drift 자체가 원천 차단. Rust 테스트는 불필요 (include_str 실패 시 컴파일 불가).

---

## 4. CLAUDE.md / AGENTS.md / GEMINI.md 에도 확장

현재 상태:
- `conventions_sync.rs` 가 CLAUDE.md 를 주로 sync
- AGENTS.md, GEMINI.md 는 엔진별 파생

제안:
- 각 conventions 파일 YAML frontmatter 에 `agent_template_version: "2026-04-22"` 추가
- `conventions_sync` 가 sync 할 때 버전 비교 → drift 발생 시 UI 경고
- `documentVersioningPolicy` 에 "agent-facing docs" 섹션 추가

---

## 5. 구현 순서 (별도 plan 승격 시)

1. **Phase A**: `_meta/agents-snapshot.json` 스키마 + 읽기/쓰기 유틸 (`src-tauri/src/commands/project_tools/snapshot.rs`).
2. **Phase B**: `ensure_workflow_templates` 를 옵션 B (`include_str!`) 로 리팩. drift 원천 차단.
3. **Phase C**: 로컬 수정 보존 로직 (sha256 비교 → 자동/수동 분기).
4. **Phase D**: UI — Settings 또는 프로젝트 헤더에 "템플릿 업그레이드 가능" 배지.
5. **Phase E**: CLAUDE.md / AGENTS.md / GEMINI.md 에도 확장, `documentVersioningPolicy` 갱신.

---

## 6. Scope 경계

- **일반 docs/ 문서는 대상 아님** — `documentVersioningPolicy` 가 이미 `updated_at` 메타 방식으로 처리. 본 idea 는 **에이전트가 로드·참조하는 고정 포맷 문서** 에만 해당.
- **Skills snapshot 은 이미 잘 동작** → 재작업 X. 스키마 참고만.
- **사용자의 전체 문서 버전 관리 거버넌스** 는 본 idea 의 범위 밖. 에이전트 facing 에만 집중.

---

## 7. 검증 필요 항목

- `include_str!` 로 바꿔도 번들 크기 증가 무시 가능 수준인지 (3 파일 합쳐 ~10KB 추정)
- 로컬 수정 보존 로직에서 merge conflict 수동 UI 가 얼마나 필요한지 (실사용 빈도 예측)
- `conventions_sync` 와 `_meta/agents-snapshot.json` 의 중복 메타 우려 — 통합 메타로 갈지 분리 유지

---

## 8. 관련 문서

- `src-tauri/src/commands/project_tools.rs` — 현재 템플릿 상수 + ensure_workflow_templates
- `src-tauri/src/commands/conventions_sync.rs` — CLAUDE.md sync 메커니즘
- `scripts/publish-skills.sh` — skills snapshot 발행 (참고 대상)
- `docs/reference/documentVersioningPolicy_2026-03-30.md` — 기존 정책 (확장 대상)
- PR #117 — drift 임시 수정 (본 idea 이전의 stop-gap)
