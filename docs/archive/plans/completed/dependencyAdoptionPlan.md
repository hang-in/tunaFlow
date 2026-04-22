# Dependency Adoption Plan — 고영향 의존성 순차 도입

> type: plan
> status: draft
> updated_at: 2026-03-31

---

## 원칙

1. **한 번에 하나만 도입** — 의존성 추가 → 빌드 확인 → 기능 검증 → 커밋 → 다음
2. **기존 동작 보존 우선** — 새 의존성을 도입하되 기존 코드는 최소 변경. 점진적 마이그레이션
3. **각 단계마다 rollback 가능** — 커밋 단위가 작아서 `git revert` 한 번으로 되돌릴 수 있어야 함
4. **빌드+테스트 게이트** — 매 단계: `cargo check` → `cargo test --lib` → `tsc --noEmit` → `vitest run`

---

## Phase 1: 플러그인 추가 (사이드이펙트 없음)

설치만 하고 기존 코드 변경 없음. 가장 안전.

### Step 1-1: tauri-plugin-clipboard-manager

**목적**: `navigator.clipboard` → 네이티브 클립보드 (포커스 없을 때도 동작)

```bash
cargo add tauri-plugin-clipboard-manager
npm install @tauri-apps/plugin-clipboard-manager
```

**변경 범위**:
- `src-tauri/src/lib.rs`: `.plugin(tauri_plugin_clipboard_manager::init())` 추가
- `src-tauri/capabilities/default.json`: `"clipboard-manager:default"` 추가
- 기존 코드 변경: **없음** (다음 단계에서 점진 마이그레이션)

**검증**: 빌드 통과 확인. 기존 복사 기능 동작 확인.

### Step 1-2: tauri-plugin-shell

**목적**: CLI 에이전트 프로세스 관리 안전성 향상

```bash
cargo add tauri-plugin-shell
npm install @tauri-apps/plugin-shell
```

**변경 범위**:
- `src-tauri/src/lib.rs`: `.plugin(tauri_plugin_shell::init())` 추가
- `src-tauri/capabilities/default.json`: `"shell:default"` 추가
- 기존 코드 변경: **없음** (에이전트 코드 마이그레이션은 별도 Phase)

**검증**: 빌드 통과 확인. 에이전트 실행 정상 동작 확인.

### Step 1-3: tauri-plugin-opener

**목적**: 메시지 내 파일 경로 클릭 → OS 앱으로 열기

```bash
cargo add tauri-plugin-opener
npm install @tauri-apps/plugin-opener
```

**변경 범위**:
- `src-tauri/src/lib.rs`: `.plugin(tauri_plugin_opener::init())` 추가
- `src-tauri/capabilities/default.json`: `"opener:default"` 추가
- 기존 코드 변경: **없음**

**검증**: 빌드 통과 확인.

---

## Phase 2: Rust crate 추가 (의존성만, 코드 변경 없음)

### Step 2-1: chrono

**목적**: `now_epoch()` 수동 구현 → 표준 시간 처리

```bash
cargo add chrono --features serde
```

**변경 범위**:
- `Cargo.toml`에 추가
- 기존 코드 변경: **없음** (점진 마이그레이션은 Phase 4에서)

**검증**: `cargo check` 통과.

### Step 2-2: tokio

**목적**: async runtime 도입 (에이전트 실행 제어의 근본 개선)

```bash
cargo add tokio --features rt-multi-thread,sync,time,process
```

**⚠️ 중요**: tokio 추가만 하고, 기존 `std::thread::spawn` 코드는 **건드리지 않는다**. Tauri 2가 이미 내부적으로 tokio를 사용하므로 의존성 충돌 없음.

**변경 범위**:
- `Cargo.toml`에 추가
- 기존 코드 변경: **없음**

**검증**: `cargo check` + `cargo test --lib` 통과.

---

## Phase 3: Frontend 라이브러리 추가 (코드 변경 없음)

### Step 3-1: react-virtuoso

**목적**: 200+ 메시지 가상 스크롤

```bash
npm install react-virtuoso
```

**변경 범위**:
- `package.json`에 추가
- 기존 코드 변경: **없음** (ChatPanel 마이그레이션은 별도)

### Step 3-2: cmdk

**목적**: 커맨드 팔레트 (에이전트 전환, 대화 검색)

```bash
npm install cmdk
```

**변경 범위**: `package.json`에 추가만. 기존 코드 변경 없음.

### Step 3-3: sonner

**목적**: 토스트 알림 (scaffold 완료, 에러 표시)

```bash
npm install sonner
```

**변경 범위**: `package.json`에 추가만. 기존 코드 변경 없음.

---

## Phase 4: 점진적 마이그레이션 (기존 코드 수정 — 한 파일씩)

Phase 1-3에서 설치한 의존성을 실제로 사용하기 시작. **각 step이 독립 커밋이어야 함.**

### Step 4-1: clipboard 마이그레이션

**대상**: `navigator.clipboard.writeText()` 호출부

**변경**:
- `MessageActions.tsx`: 복사 버튼 → `writeText()` from `@tauri-apps/plugin-clipboard-manager`
- `RtMessageCard.tsx`: 같은 패턴
- `MarkdownComponents.tsx`: 코드 블록 복사 버튼

**rollback**: 이전 커밋의 `navigator.clipboard` 호출로 복원 가능

### Step 4-2: sonner 토스트 도입

**대상**: 현재 시스템 메시지로 표시하는 알림

**변경**:
- `AppShell.tsx`: `<Toaster />` 마운트
- `ProjectStartup.tsx`: scaffold 알림 → `toast()` 호출
- 에러 표시: `set({ error })` → `toast.error()`

**rollback**: `<Toaster />` 제거 + 이전 알림 방식 복원

### Step 4-3: react-virtuoso 도입

**대상**: `ChatPanel.tsx`의 메시지 렌더링

**변경**:
- `messages.map(...)` → `<Virtuoso data={messages} itemContent={...} />`
- auto-scroll 로직 조정 (`followOutput` prop)

**⚠️ 주의**: 메시지 그룹핑, 스트리밍 상태, 스크롤 위치 복원에 영향. 충분한 테스트 필요.

**rollback**: `<Virtuoso>` → `messages.map()` 복원

### Step 4-4: cmdk 커맨드 팔레트

**대상**: 새 컴포넌트 추가 (기존 코드 수정 최소)

**변경**:
- `CommandPalette.tsx` 신규 생성
- `AppShell.tsx`: Cmd+K 바인딩 + `<CommandPalette />` 마운트
- 액션: 프로젝트 전환, 대화 전환, 엔진 전환

**rollback**: 컴포넌트 + 바인딩 제거

---

## Phase 5: tokio 마이그레이션 (대규모 — 별도 세션 권장)

**⚠️ 가장 리스크 큰 단계.** 에이전트 실행 경로 전체에 영향.

### 전략: Dual-path 전환

기존 `std::thread::spawn` 코드를 **즉시 삭제하지 않고**, 새 tokio 경로를 나란히 만들어 검증 후 전환.

### Step 5-1: tokio runtime 초기화

**변경**:
- `lib.rs`에서 Tauri의 기존 tokio runtime 활용 확인
- 새 `async fn` 하나를 테스트로 추가하여 tokio가 동작하는지 확인

### Step 5-2: `start_claude_stream` async 전환

**변경**:
- `start_claude_stream_v2` async 버전 추가 (기존 함수 유지)
- 프론트엔드에서 v2 호출로 전환
- 기존 함수 확인 후 제거

### Step 5-3: 나머지 엔진 async 전환

- Gemini, Codex, OpenCode 동일 패턴
- 각 엔진마다 독립 커밋

### Step 5-4: RT executor async 전환

- `execute_sequential` / `execute_parallel` → tokio task 기반
- `CancellationToken` 도입 → 참가자 강제 종료 가능

### Step 5-5: 정리

- 기존 sync 함수 제거
- `std::thread::spawn` 사용처 0개 확인
- `CancelRegistry` → `CancellationToken` 전환

---

## 검증 게이트 (매 Step 후)

```bash
cd src-tauri && cargo check                    # Rust 컴파일
cd src-tauri && cargo test --lib               # 57+ 테스트
cd .. && npx tsc --noEmit                      # TypeScript
cd .. && npx vitest run                        # 55+ 프론트엔드 테스트
# tauri dev로 수동 확인 (에이전트 전송, RT, 설정 등)
```

---

## 리스크 매트릭스

| Phase | 리스크 | 영향 범위 | rollback |
|-------|--------|----------|----------|
| 1 (플러그인 설치) | **극히 낮음** | Cargo.toml + lib.rs만 | cargo remove |
| 2 (Rust crate 설치) | **극히 낮음** | Cargo.toml만 | cargo remove |
| 3 (npm 설치) | **극히 낮음** | package.json만 | npm uninstall |
| 4-1~4-2 (clipboard/sonner) | **낮음** | UI 컴포넌트 2-3개 | git revert |
| 4-3 (virtuoso) | **중간** | ChatPanel 전체 | git revert (주의: 스크롤 동작 차이) |
| 4-4 (cmdk) | **낮음** | 새 컴포넌트 추가만 | 파일 삭제 |
| 5 (tokio) | **높음** | 에이전트 실행 경로 전체 | dual-path로 점진 전환 |

---

## 타임라인 예상

| Phase | 예상 시간 | 선행 조건 |
|-------|----------|----------|
| 1-3 (설치만) | 10분 | 없음 |
| 4-1~4-2 | 30분 | Phase 1 |
| 4-3 | 1시간 | Phase 3 |
| 4-4 | 1시간 | Phase 3 |
| 5 (tokio) | 별도 세션 | Phase 2 |
