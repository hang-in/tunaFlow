---
title: 프로젝트별 DB 분리 + 외부 파일 레퍼런스 정책
status: planned
priority: P1   # 베타 정식 공개 전 권장. 베타 4/RC 시점과 bundle identifier 변경과 함께 묶기
created_at: 2026-04-16
related:
  - docs/ideas/projectPerWindowIdea.md
  - docs/plans/projectScopedConcurrencyPlan.md
  - docs/reference/dataModelRevised.md
  - src-tauri/src/db/migrations.rs
  - ~/.tunaflow/memory/project_db_architecture.md  (AI memory)
---

# 프로젝트별 DB 분리 + 외부 파일 레퍼런스 정책

## 1. 결정 (확정)

사용자 방향 확정:

- **크로스 프로젝트 세션/대화/retrieval은 검색하지 않는다.** 프로젝트 A에서 프로젝트 B의 대화, 브랜치, chunk, plan, artifact, trace를 retrieval 대상에 넣지 않음.
- **다른 프로젝트의 파일은 읽기 전용으로 참조 가능.** 예: `~/privateProject/_research/_util/` 같은 레퍼런스 라이브러리를 rawq 코드 검색이나 파일 Read 도구에서 활용.
- 따라서 DB는 **프로젝트별 분리** (각자 독립 DB 파일). 단일 DB + `project_key` 필터 전략은 **채택하지 않음** — 필터 실수 시 격리가 깨지고, 격리가 확정이면 처음부터 파일 수준에서 나누는 게 명확함.

## 2. 왜 분리가 맞는가

| 이유 | 설명 |
|------|------|
| 격리 의도 반영 | 쿼리 실수로 크로스 프로젝트 데이터가 새는 경로가 구조적으로 제거 |
| 삭제/이동 단순 | 프로젝트 삭제 = 해당 DB 파일 하나만 삭제. 현재는 `DELETE ... WHERE project_key=?` 수십 개 필요 (실측 `conversations.rs:123` `branches.rs:413` 등) |
| 마이그레이션 단위화 | 스키마 버전 up 실패가 다른 프로젝트로 번지지 않음 |
| 용량/백업 관리 | 큰 프로젝트만 별도 백업/압축/아카이브 가능 |
| project-per-window와 자연스럽게 맞물림 | 윈도우=프로젝트=DB 1:1:1로 정리 (`projectPerWindowIdea.md` §2) |
| 크로스 프로젝트 검색이 없다는 전제 하에 조인 손실은 무의미 | 사용자 결정으로 trade-off 없어짐 |

## 3. 구조 설계

### 3.1 파일 레이아웃

```
~/Library/Application Support/<bundle-identifier>/
├── meta.db                             # 전역 메타 (프로젝트 목록, user profile, 앱 설정)
├── projects/
│   └── <project_key>/
│       ├── tunaflow.db                 # 프로젝트 DB (대화/브랜치/RT/plan/chunk/trace 전부)
│       ├── tunaflow.db-wal
│       └── tunaflow.db-shm
├── skills/                             # 전역 (기존 그대로)
└── logs/
```

`<bundle-identifier>`는 앞서 논의된 변경 예정값 (예: `dev.d9ng.tunaflow`). bundle identifier 변경 마이그레이션과 **한 번에 묶어** 처리.

### 3.2 meta.db 스키마 (전역)

```sql
CREATE TABLE projects (
  key TEXT PRIMARY KEY,
  label TEXT,
  path TEXT NOT NULL,
  db_path TEXT NOT NULL,                -- projects/<key>/tunaflow.db 상대 또는 절대 경로
  created_at INTEGER NOT NULL,
  hidden INTEGER DEFAULT 0,
  -- 기존 projects 테이블의 onboarding 등 전역 필드들 여기로
  meta_conversation_id TEXT,
  onboarding_done INTEGER DEFAULT 0
);

CREATE TABLE user_profile (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  name TEXT, title TEXT, bio TEXT,
  preferred_languages TEXT,
  git_name TEXT, git_email TEXT,
  github_org TEXT,
  updated_at INTEGER NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE external_file_refs (          -- 3.4 참조
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  owner_project_key TEXT NOT NULL,
  external_path TEXT NOT NULL,
  label TEXT,
  read_only INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  FOREIGN KEY (owner_project_key) REFERENCES projects(key) ON DELETE CASCADE
);
```

### 3.3 프로젝트 DB 스키마

**현재 단일 DB의 거의 전부가 프로젝트 DB로 이동**:

- `conversations`, `messages`, `messages_fts`, `branches`, `memos`, `artifacts`
- `plans`, `plan_events`, `plan_subtasks`, `failure_lessons`
- `conversation_chunks`, `vec_chunks`
- `trace_log`, `agent_jobs`, `session_links`, `session_discovery`
- `rt_configs`, `conversation_memory`, 기타 도메인 테이블 전부

**project_key 컬럼 제거 가능** (DB 자체가 프로젝트 하나 전용이므로). 점진적 제거 권장 — 호환성 깨짐 방지.

### 3.4 외부 파일 레퍼런스 정책

사용자 요건: "다른 프로젝트의 파일은 레퍼런스로 읽기 전용 사용 가능" (예: `_research/_util/`).

#### 설정 UI
Settings → Project → External References:
```
[ + Add external path ]
✓ ~/privateProject/_research/_util        (read-only)
✓ ~/Documents/code-snippets               (read-only)
```

#### 런타임 보장
1. **rawq 인덱스 대상 확장**: 현재 프로젝트 path + 허용된 external paths
2. **파일 Read 도구**: 현재 프로젝트 내부 OR 허용 리스트 prefix 매치만 허용
3. **Write 도구 금지**: external path는 항상 read-only. Developer 에이전트의 Edit/Write 호출 시 path prefix 검사 → 거부
4. **격리 검증**: 테스트로 `Write(~/other-project/foo.ts)` 같은 호출이 실제 차단되는지 확인

## 4. 마이그레이션 전략

### 4.1 트리거
- bundle identifier 변경으로 새 디렉토리(`dev.d9ng.tunaflow/`)에서 시작
- 첫 실행 시 기존 경로(`com.tunaflow.app/tunaflow.db`) 존재 감지
- 사용자에게 모달 노출: "기존 데이터를 가져오시겠습니까? [Import] [Skip (fresh start)]"

### 4.2 Import 로직
```
기존 단일 DB 읽기
  ├── projects 테이블 → meta.db.projects 로 복사
  │                    + db_path = projects/<key>/tunaflow.db 할당
  ├── user_profile, app_settings → meta.db
  └── 각 project_key 별로:
      1. projects/<key>/tunaflow.db 생성
      2. 해당 project_key 조건에 맞는 모든 도메인 테이블 row 복사
      3. 복사 중 conversation_id/branch_id/plan_id 등 UUID는 그대로 유지
      4. vec_chunks는 conversation_chunks rowid 기준 재생성 필요 (가상 테이블 INSERT)
      5. messages_fts 재생성 (PRAGMA rebuild)
      6. 성공 시 쓰기 잠금 해제 + 다음 프로젝트로 이동
원본 단일 DB는 `com.tunaflow.app/tunaflow.db.legacy-pre-split` 로 rename만 (삭제 X)
```

### 4.3 실패 처리
- 프로젝트 단위로 atomic: 한 프로젝트 import 실패 시 해당 DB 파일 삭제하고 다음 프로젝트 진행
- 모두 끝난 뒤 실패 목록을 사용자에게 보고 + `tunaflow.db.legacy-pre-split` 보존 상태 안내 (수동 복구 여지)

### 4.4 Fresh start 옵션
베타 1~3은 개인 테스트 규모라 "Fresh start" 선택지를 제공 — 데이터 없이 시작. 빠르고 안전함.

## 5. API 레이어 변경

### 5.1 DbState 전환
현재:
```rust
pub struct DbState {
    pub read: Arc<Mutex<Connection>>,
    pub write: Arc<Mutex<Connection>>,
}
```

변경:
```rust
pub struct DbState {
    pub meta: Arc<Mutex<Connection>>,          // meta.db read/write
    pub projects: Arc<RwLock<HashMap<String, ProjectConnections>>>,
}

pub struct ProjectConnections {
    pub read: Arc<Mutex<Connection>>,
    pub write: Arc<Mutex<Connection>>,
    pub opened_at: Instant,
}
```

- 프로젝트 선택 시 lazy open + cache
- 일정 시간 idle(예: 10분) + 다른 윈도우에서 안 쓰면 close → 메모리 관리
- project-per-window 시나리오: 윈도우 당 1~2 프로젝트 커넥션만 살아있음

### 5.2 Tauri command 시그니처
거의 모든 커맨드가 `conversation_id`로 시작 → conversation_id 포맷에 **project_key prefix 포함**하거나, 별도 인자 추가.

대안 1: `conversation_id`는 UUID만 유지, command에 `project_key` 인자 추가
대안 2: 현재 state에 "active project" 걸어두고 모든 커맨드가 active로 라우팅 (window-per-project 시 자연스러움)

**권장**: 대안 2 (window-per-project와 맞음). `AppState::active_project_key: Arc<RwLock<String>>` 추가하고 커맨드 내부에서 resolve.

### 5.3 HTTP API (외부 MCP 등)
MCP 서버 경로는 `project_key`를 path에 명시: `/api/projects/<key>/...` 형식. 현재 `/api/conversations/<id>` → conversation 조회 시 메타 DB에서 project_key 역조회 후 해당 DB 열기.

## 6. bundle identifier 변경과 묶기

두 변경은 **같은 베타 태그(v0.1.0-beta.4)에서 한 번에 릴리즈**:

1. `tauri.conf.json`: `identifier` 변경 (`com.tunaflow.app` → `dev.d9ng.tunaflow` 등)
2. Application Support 경로 전환
3. DB 분리 + 마이그레이션
4. 릴리즈 노트에 "기존 베타 데이터는 1회 마이그레이션 또는 Fresh start" 공지

한 번에 하면 사용자가 경로 변경을 한 번만 체감. 분리하면 두 번 마이그레이션 경험 → 혼란.

## 7. 단계별 로드맵

| 단계 | 내용 | 작업량 |
|------|------|--------|
| 0 | 이 plan 검토 + 스키마 초안 합의 | 0.5일 |
| 1 | meta.db 스키마 + DbState 전환 (프로젝트 0~1개 상태로 동작) | 1~1.5일 |
| 2 | 프로젝트 DB 생성/열기/닫기 + connection cache | 0.5~1일 |
| 3 | 모든 command을 active project 기반 라우팅으로 교체 | 2일 |
| 4 | 마이그레이션 코드 (legacy 단일 DB → 분리) + UI 모달 | 1~1.5일 |
| 5 | external_file_refs UI + rawq 인덱스 범위 확장 + Write prefix 검증 | 1일 |
| 6 | bundle identifier 변경 + 릴리즈 노트 작성 | 0.5일 |
| 7 | 테스트 (기존 베타 DB 샘플로 마이그레이션 드라이런) | 0.5~1일 |

**합 ~8~9일**. 베타 정식 공개 전 작업으로 현실적. RT 고도화 sprint 다음에 위치시키는 게 우선순위상 맞음.

## 8. 남은 질문

- bundle identifier 최종 값: `dev.d9ng.tunaflow` vs `io.tunaflow.desktop` vs 기타
- `external_file_refs` 를 프로젝트 DB에 둘지 meta.db에 둘지 — **meta.db 제안** (프로젝트 파괴돼도 레퍼런스는 전역 설정으로 유지 가능하게)
- project-per-window를 DB 분리와 **같이** 할지 **후속**으로 할지. 내 의견: **DB 분리만 먼저**, window 분리는 베타 후 P2. 두 변경을 묶으면 복잡도 급증
- Fresh start 선택 시 기존 legacy DB 파일을 자동 백업하고 언제 삭제할지 — 기본값 **삭제 안 함**, 사용자 수동 정리

## 9. 관련 문서

- `docs/ideas/projectPerWindowIdea.md` — window 분리 아이디어 (DB 분리의 상위 컨텍스트)
- `docs/plans/projectScopedConcurrencyPlan.md` — 프로젝트별 동시성 (DB 분리 후 락 설계 단순화 효과)
- `docs/reference/dataModelRevised.md` — 도메인 모델 SSOT (스키마 변경 시 갱신 필요)
- `docs/plans/betaReleaseReadinessPlan.md` — 베타 체크리스트
