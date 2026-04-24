---
title: rawq index build 시 빌드 산출물 디렉터리 제외 (Issue #180 hotfix)
status: ready-to-implement
priority: P0 (베타 blocker — 사용자 시스템 프리즈 유발)
created_at: 2026-04-24
related:
  - https://github.com/dghong/tunaFlow/issues/180
  - docs/plans/sidecarPipelinePlan_2026-04-24.md
  - docs/plans/postBetaBacklogPlan_2026-04-24.md  # B-1 staleness 와 인접 주제
canonical: true
owners:
  - architect (본 문서 작성)
  - developer (구현)
---

# 배경

공개 첫날 커뮤니티 사용자 `batmania52` 가 제보 (#180):

> `start_rawq_index` 가 프로젝트 오픈 시 자동 발동 → `.gitignore` 의 `target/` (16GB Rust 빌드 산출물) 을 그대로 인덱싱 시도 → **RAM 고갈 + 30GB swap 사용 → 시스템 프리즈 → 강제 리부트**.

원인: `rawq::ensure_index()` 가 `rawq index build <path> --json` 만 호출 (exclude flag 없음). 코드 주석에 **"rawq's WalkBuilder respects .gitignore automatically"** 라고 적혀 있지만 실측상 **동작 안 함**. 사용자가 증명한 실제 시나리오 두 건:

1. `utils-project` — `.gitignore` 에 `src-tauri/target/` 포함했는데 16GB target 인덱싱 시도 → 시스템 freeze
2. `chrome-extensions` — `.gitignore` 에 `node_modules/` 포함했는데 인덱싱됨 (76MB, 임팩트 작음)

**영향 범위**: Rust 프로젝트를 열면 **모든 사용자가 잠재적으로 시스템 freeze**. 진행 중이던 작업 손실 위험.

rawq upstream 에도 장기적 수정이 필요하지만 (`.gitignore` 존중이 원래 claim), 공개 배포판은 **방어적 hardcoded exclude** 로 즉시 차단한다.

# 현재 상태 (사실 확인)

## (A) `src-tauri/src/agents/rawq.rs:320-369` `ensure_index`

```rust
// Line 336-339
let bin = resolve_rawq_bin()?;
// Note: rawq's WalkBuilder respects .gitignore automatically.
// Explicit -x patterns are not needed for standard ignores.
let child = Command::new(&bin)
    .args(["index", "build", project_path, "--json"])
```

- 주석의 가정이 **틀렸음** (이슈 #180 으로 반증)
- exclude flag 없음 → rawq 가 target/ / node_modules/ 등 전부 긁음

## (B) 호출 경로 두 개 (모두 같은 함수 호출)

- `src-tauri/src/commands/project_tools.rs:61` `ensure_rawq_index` (blocking)
- `src-tauri/src/commands/project_tools.rs:122` `start_rawq_index` (background)

**둘 다 `ensure_index` 를 통하므로 수정은 한 곳만** — 라이브러리 함수 내부에 hardcoded exclude 를 넣는 편이 호출부 누락 위험이 없다.

## (C) rawq CLI 의 exclude 지원

- `rawq index build <path> --json -x PATTERN` — glob 지원 (이슈 #180 본문 기준)
- `-x` 는 반복 가능 (`-x target/** -x node_modules/**`)

## (D) 기존 인덱스 DB (레거시 오염)

이미 한 번이라도 프로젝트를 연 사용자는 **target/ 이 이미 인덱싱된 DB** 를 가질 수 있다. hotfix 가 머지돼도 이 레거시는 자동 정리 안 됨. 두 가지 방침 중 선택:

- **A 안**: 버전 bump 로 자동 rebuild — rawq 에 `--force` 플래그가 있다면 앱 업데이트 후 한 번 재인덱싱. 구현 복잡.
- **B 안**: 사용자가 수동 재인덱싱 UI 노출 ("Rebuild index" 버튼 — Settings 또는 rawq 상태 아이콘).

B 안이 hotfix 로 적정. A 안은 후속 plan.

# 설계 (MVP — hotfix 1일)

## (1) `ensure_index` 에 hardcoded exclude 추가

**파일**: `src-tauri/src/agents/rawq.rs:336-339`

```rust
// 변경 전
let bin = resolve_rawq_bin()?;
// Note: rawq's WalkBuilder respects .gitignore automatically.
// Explicit -x patterns are not needed for standard ignores.
let child = Command::new(&bin)
    .args(["index", "build", project_path, "--json"])

// 변경 후
let bin = resolve_rawq_bin()?;
// rawq WalkBuilder .gitignore 지원이 실측상 신뢰 불가 (#180). 공통 빌드
// 산출물은 하드코딩으로 제외해 OOM / 시스템 프리즈를 방어한다.
let child = Command::new(&bin)
    .args([
        "index", "build", project_path, "--json",
        "-x", "target/**",          // Rust
        "-x", "node_modules/**",    // Node
        "-x", "dist/**",            // FE 빌드 산출물
        "-x", "build/**",           // 일반 빌드
        "-x", ".venv/**",           // Python
        "-x", "venv/**",            // Python 변형
        "-x", "__pycache__/**",     // Python
        "-x", ".next/**",           // Next.js
        "-x", ".nuxt/**",           // Nuxt
        "-x", ".cache/**",          // 일반 캐시
        "-x", "coverage/**",        // 테스트 커버리지
        "-x", "*.min.js",           // 미니파이
        "-x", "*.min.css",
        "-x", "*.lock",             // lockfile
        "-x", "*.log",
    ])
```

**패턴 선정 근거**: tunaFlow 자체 프로젝트와 흔히 혼합되는 스택 (React + Node + Rust + Python) 기준. 너무 공격적이면 실제 소스 누락 리스크 → **재현 빈도 높은 8~10개만**.

## (2) 수동 재인덱싱 UI (선택 — 권장)

기존 사용자의 오염된 DB 정리용. 범위가 커지면 분리 가능하지만 hotfix 와 함께 넣는 편이 사용자 안내가 깔끔.

**신규 함수**: `src-tauri/src/agents/rawq.rs` 에 `rebuild_index(project_path: &str)` 추가

```rust
/// 기존 인덱스를 삭제하고 재빌드. #180 hotfix 후 레거시 오염 정리용.
pub fn rebuild_index(project_path: &str) -> Result<u64, RawqError> {
    let bin = resolve_rawq_bin()?;
    // Step 1: 기존 인덱스 drop
    let _drop = Command::new(&bin)
        .args(["index", "drop", project_path, "--json"])
        .output()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;
    // drop 실패는 무시 (처음부터 인덱스 없을 수 있음)

    // Step 2: ensure_index 재호출
    ensure_index(project_path)
}
```

**rawq CLI `index drop` 명령 존재 확인 필요** — 없다면 DB 파일 경로 찾아서 수동 삭제 (범위 초과 → 이 경우 rebuild UI 는 후속 plan 으로 분리).

**신규 Tauri command**: `project_tools.rs` 에 `rebuild_rawq_index` 추가 (`start_rawq_index` 와 동일한 background 패턴).

**UI**: Settings 또는 rawq 상태 아이콘 우클릭 메뉴에 "인덱스 재빌드" 버튼. 확인 dialog 필수 ("시간 오래 걸릴 수 있습니다").

## (3) README / 문서 업데이트

- README 의 rawq 섹션에 hardcoded exclude 목록 명시
- `docs/how-to/rawq-setup.md` 에 "제외 패턴" 섹션 신규

## (4) 테스트

### Rust 유닛 테스트 — 실현 불가

`ensure_index` 는 실제 rawq 바이너리를 호출하므로 mock 이 어렵다. 대신:

- **integration test (최소)**: 테스트 프로젝트 디렉터리에 `target/` 폴더 하나 만들고 내부에 몇 MB 더미 파일 넣은 뒤 `ensure_index` → `rawq search` 로 `target/` 경로 결과 0건 확인. `#[ignore]` 로 표시하고 수동 실행.
- **CI 에는 넣지 않음** (rawq 바이너리 필요 + 시간 소요)

### 수동 검증 시나리오 (PR 검증용)

1. 16GB 이상 `target/` 를 가진 Rust 프로젝트 열기
2. Activity Monitor 로 tunaFlow 메모리 관찰 → 500MB 이내 유지 확인
3. `rawq search "main"` → `target/` 경로 결과 **0 건**
4. `rawq search "src"` → 실제 소스 결과는 정상 반환

# Invariants

- **[INV-1]** `target/`, `node_modules/`, `.venv/` 아래 파일은 **절대 rawq 인덱스에 포함되지 않는다**. 검증: hardcoded 패턴 grep + 수동 search 테스트.
- **[INV-2]** hardcoded exclude 패턴은 `ensure_index` 한 함수 안에만 존재한다 (다른 rawq 호출 경로가 생기면 같이 적용돼야 함을 컨벤션으로). 검증: `rg '"index", "build"' src-tauri/src/` 결과 1건.
- **[INV-3]** exclude 패턴 변경은 **추가만** (기존 패턴 삭제 금지) — 한 번 제외한 디렉터리를 다시 포함하면 기존 사용자 DB 에 갑자기 대량 인덱싱 유발. 삭제가 필요하면 별도 plan.
- **[INV-4]** rebuild 기능을 구현하면 UI 에 **반드시 확인 dialog** 필요. 실수 클릭으로 장시간 재인덱싱 유발 방지.

# Rationale

## 왜 rawq upstream 수정이 아니라 tunaFlow 방어 코드인가

- upstream `.gitignore` 버그 수정은 rawq 내부 WalkBuilder 설정 문제 → **외부 의존 + 릴리즈 주기 알 수 없음**
- 사용자 시스템이 **지금 얼어붙고 있음** → 최단 시간 hotfix 필요
- tunaFlow 방어 코드 제거는 upstream fix 확정 후 (이 plan 머지 시 `postBetaBacklog` 에 "upstream fix 후 hardcoded exclude 제거" 항목 추가)

## 왜 .gitignore 동적 읽기가 아니고 hardcoded 인가

- `.gitignore` 파서 구현 비용 무시 못함 (nested .gitignore, negation 패턴, `!` prefix 등)
- rawq 가 이미 `.gitignore` 를 읽는데 안 지키는 상황 → 파서 하나 더 만든다고 신뢰성 올라가지 않음
- hardcoded 는 **명확하고 리뷰 가능** — 어떤 디렉터리가 제외되는지 코드만 봐도 안다
- 동적 `.gitignore` 파싱은 rawq upstream 으로 넘긴다

## 왜 패턴 수가 10~15개로 제한적인가

너무 공격적이면 (`*.json`, `*.md` 같은) 사용자 실 소스 누락 → 검색 결과 품질 저하. 현재 목록은 **빈도 높은 빌드 산출물 + lockfile + 로그** 로 제한. 추가 요청 있으면 개별 plan.

## 왜 rebuild UI 가 hotfix 와 같이 가는가

- hardcoded exclude 머지 + 재배포만 해서는 **기존 사용자 DB 에 target/ 청크가 그대로 남음**
- 검색 결과에 target/ 경로가 계속 나와 "이 fix 안 먹힌거 아닌가" 오해 유발
- rebuild 버튼 하나로 사용자가 스스로 해결 가능 → 고객 지원 비용 ↓

# Developer 핸드오프 프롬프트

```
[작업] rawq 인덱싱 시 빌드 산출물 디렉터리 제외로 OOM / 시스템 프리즈 방지 (Issue #180 hotfix)

[SSOT] /Users/d9ng/privateProject/tunaFlow/docs/plans/rawqGitignoreIndexFixPlan_2026-04-24.md 먼저 읽고 설계 §(1)~(4) 순서대로 처리.

[배경 3줄]
- 공개 첫날 사용자 시스템 프리즈 + 강제 리부트 발생
- rawq 가 .gitignore 를 실제로 존중하지 않아 target/ 16GB 인덱싱 시도
- 방어적 hardcoded exclude 로 즉시 차단 + 기존 사용자용 rebuild UI

[수정 범위]

1) 수정: src-tauri/src/agents/rawq.rs
   - ensure_index (line 320) 안의 Command::new args 에 -x 패턴 하드코딩 추가
   - 패턴 목록은 SSOT §(1) 참조 (14개: target/**, node_modules/**, dist/**, build/**,
     .venv/**, venv/**, __pycache__/**, .next/**, .nuxt/**, .cache/**, coverage/**,
     *.min.js, *.min.css, *.lock, *.log)
   - 기존 주석 "rawq's WalkBuilder respects .gitignore automatically..." 를
     "rawq WalkBuilder .gitignore 지원이 실측상 신뢰 불가 (#180). 공통 빌드 산출물은
     하드코딩으로 제외해 OOM / 시스템 프리즈를 방어한다." 로 교체
   - 새 함수 rebuild_index(project_path: &str) 추가 (index drop 시도 → ensure_index 재호출)

2) 수정: src-tauri/src/commands/project_tools.rs
   - 신규 Tauri command rebuild_rawq_index — start_rawq_index 와 동일 패턴
     (background thread + 이벤트 3종 emit). 중복 방지 guard 재사용.
   - main.rs 의 tauri::Builder invoke_handler 에 등록

3) 수정: src/lib/api/rawq.ts (또는 해당 경로)
   - 신규 API 함수: rebuildRawqIndex(projectPath: string): Promise<void>
   - invoke("rebuild_rawq_index", { projectPath }) 호출

4) 수정: Settings 또는 rawq 상태 아이콘 컴포넌트
   - "인덱스 재빌드" 버튼 추가
   - 클릭 시 confirm dialog: "기존 인덱스를 삭제하고 다시 빌드합니다. 프로젝트 크기에 따라
     수 분이 걸릴 수 있습니다. 계속하시겠습니까?"
   - 확인 시 rebuildRawqIndex 호출 + toast "재빌드 시작" + 상태는 기존 이벤트 구독으로 갱신

5) 수정: README.md + docs/how-to/rawq-setup.md
   - README: rawq 섹션에 "제외 패턴" 짧게 명시
   - how-to: "제외 패턴" 신규 섹션 — 전체 목록 + 추가 요청 시 프로세스

6) 테스트
   - Rust 유닛 테스트는 rawq 바이너리 필요로 #[ignore] 마킹. CI 포함 금지.
   - 수동 검증 시나리오는 SSOT §(4) 참조 — PR 본문에 실행 결과 캡처 첨부

[검증]
- cd src-tauri && cargo check --all-targets: 0 에러
- npx tsc --noEmit: 0 에러
- 수동 smoke:
    1. Rust 프로젝트 (target/ 3GB+) 열기 → Activity Monitor 로 메모리 500MB 이내 확인
    2. rawq 상태 아이콘 "인덱스 재빌드" 버튼 → confirm dialog → 실행 완료
    3. rawq search "fn main" → target/ 경로 결과 0건 확인

[커밋]
- fix(rawq): exclude build artifact dirs from ensure_index (#180)
- feat(rawq): rebuild_index command + UI button for cleaning legacy DB
- docs(rawq): exclude pattern documentation

각 커밋 trailer 에 Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
fix(rawq): prevent OOM by excluding build artifact dirs from index (#180)

PR 본문에 시스템 freeze 재현 + hotfix 후 메모리 사용량 캡처 2장 필수.

[주의]
- git stash drop/clear 금지
- 하드코딩 패턴 삭제 금지 (INV-3) — 추가만 가능
- rebuild_index 내부 index drop 실패는 무시 (처음부터 인덱스 없을 수 있음) — 로그만 남기고 ensure_index 로 진행
- rebuild UI 는 반드시 confirm dialog — 실수 클릭 시 장시간 재인덱싱 유발 방지
- `rawq index drop` 서브커맨드가 rawq 현재 버전에 없다면: rebuild 기능은 본 PR 에서 빼고 별도 PR 로 분리. 핵심은 ensure_index hardcoded exclude (이것만으로도 신규 사용자 보호됨)
```

# 관련 기록

- Issue #180 (`batmania52`, 2026-04-24) — 원 제보
- `docs/plans/postBetaBacklogPlan_2026-04-24.md` B-3 (CRG 언어 지원) / B-7 (snapshot 신선도) — 인접 주제이지만 직접 의존 없음
- 후속 plan 후보: "rawq upstream `.gitignore` 존중 PR" — upstream fix 후 본 plan 의 hardcoded exclude 제거 검토
