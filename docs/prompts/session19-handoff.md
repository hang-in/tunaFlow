# 세션 19 핸드오프 프롬프트

> 아래 내용을 새 세션의 첫 메시지로 사용하세요.

---

tunaFlow 세션 19 시작. **HTTP API E2E 테스트 세션**.

## 프로젝트 개요

tunaFlow는 **다중 에이전트 오케스트레이션 클라이언트(AOC)**. Tauri 2 + React + TypeScript + Rust + SQLite.
프로젝트 단위로 Claude/Codex/Gemini 에이전트를 실행하며, Roundtable 토론, Branch 분기, Plan/Artifact 관리, ContextPack 맥락 조립 등을 지원한다.

## 현재 상태

- **브랜치**: `feature/context-tiering` (main에 미머지, origin push 완료)
- **세션 18 성과**: ContextPack Tiering 8항목 완료 + sqlite-vec + HTTP API Phase 1
- **테스트**: Rust 197 + Frontend 175 = 372 tests
- **DB 버전**: v30 (vec_chunks 가상 테이블)

## 이번 세션의 목표

**HTTP API를 통해 tunaFlow의 핵심 기능을 E2E 테스트**합니다.

### ⚠️ 중요 규칙

1. **코드를 수정하지 마세요.** 이 세션은 테스트 전용입니다.
2. **tunaFlow 프로젝트의 소스코드를 건드리지 마세요.** 테스트 대상은 HTTP API 엔드포인트입니다.
3. **테스트용 프로젝트 키는 `tunaInsight`를 사용하세요.** tunaFlow 자체를 테스트 데이터로 오염시키지 마세요.
4. **에이전트 실행 시 `dryRun: true`를 먼저 사용하세요.** 실제 에이전트 호출은 비용이 발생합니다.
5. **결과만 보고하세요.** 버그를 발견하면 기록만 하고 수정하지 마세요.

## HTTP API 구조

### 서버 정보
- **URL**: `http://127.0.0.1:19840`
- **인증**: `Authorization: Bearer {TOKEN}` (앱 시작 시 콘솔에 출력)
- **포맷**: JSON

### 엔드포인트 목록

#### 읽기 (상태 확인)

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/health` | 서버 상태 (인증 불필요) |
| GET | `/api/projects` | 프로젝트 목록 |
| GET | `/api/conversations?projectKey=X` | 대화 목록 |
| GET | `/api/conversations/:id/messages` | 메시지 목록 |
| GET | `/api/plans?conversationId=X` | Plan 목록 |
| GET | `/api/plans/:id` | Plan 상세 |
| GET | `/api/plans/:id/events` | Plan 이벤트 타임라인 |
| GET | `/api/artifacts?conversationId=X` | Artifact 목록 |
| GET | `/api/agents/status` | 실행 중 에이전트 상태 |

#### 쓰기 (테스트 실행)

| Method | Path | Body | 설명 |
|--------|------|------|------|
| POST | `/api/conversations` | `{"projectKey":"X","label":"Y"}` | 대화 생성 |
| POST | `/api/conversations/:id/send` | `{"prompt":"X","engine":"claude","dryRun":true}` | 메시지 전송 |
| POST | `/api/plans/:id/approve` | (없음) | Plan 승인 |

#### WebSocket

| Path | 설명 |
|------|------|
| `/ws/events` | 실시간 이벤트 (agent:completed, agent:error 등) |

## 테스트 시나리오

### Phase 1: API 기본 동작 확인

```bash
# 0. 토큰 확인 (앱 콘솔에서)
TOKEN="앱_시작_시_출력된_토큰"

# 1. 헬스체크
curl -s http://127.0.0.1:19840/api/health
# 기대: {"status":"ok","version":"0.1.0"}

# 2. 인증 실패 확인
curl -s http://127.0.0.1:19840/api/projects
# 기대: {"error":"invalid token"}

# 3. 인증 성공
curl -s http://127.0.0.1:19840/api/projects -H "Authorization: Bearer $TOKEN"
# 기대: JSON 배열 (프로젝트 목록)

# 4. 프로젝트별 대화 목록
curl -s "http://127.0.0.1:19840/api/conversations?projectKey=tunaInsight" \
  -H "Authorization: Bearer $TOKEN"
```

### Phase 2: 대화 생성 + 메시지 전송 (dry_run)

```bash
# 5. 대화 생성
CONV=$(curl -s -X POST http://127.0.0.1:19840/api/conversations \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"projectKey":"tunaInsight","label":"E2E Test Session 19"}' | jq -r '.id')
echo "Created conversation: $CONV"

# 6. dry_run 메시지 전송 (에이전트 실행 안 함)
curl -s -X POST "http://127.0.0.1:19840/api/conversations/$CONV/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"prompt":"Hello from E2E test","dryRun":true}'
# 기대: {"messageId":"...","dryRun":true,"info":"..."}

# 7. 메시지 저장 확인
curl -s "http://127.0.0.1:19840/api/conversations/$CONV/messages" \
  -H "Authorization: Bearer $TOKEN"
# 기대: user 메시지 1개 (role: "user", content: "Hello from E2E test")
```

### Phase 3: 실제 에이전트 호출 (선택적, 비용 발생)

```bash
# 8. 실제 메시지 전송 (에이전트 실행)
curl -s -X POST "http://127.0.0.1:19840/api/conversations/$CONV/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"prompt":"What is 2+2?","engine":"claude"}'
# 기대: {"messageId":"...","status":"running","info":"..."}

# 9. 에이전트 완료 대기 (폴링)
while [ "$(curl -s http://127.0.0.1:19840/api/agents/status \
  -H "Authorization: Bearer $TOKEN" | jq -r '.running')" = "true" ]; do
  echo "waiting..."; sleep 2
done

# 10. 응답 확인
curl -s "http://127.0.0.1:19840/api/conversations/$CONV/messages" \
  -H "Authorization: Bearer $TOKEN" | jq '.[-1]'
# 기대: assistant 메시지 (role: "assistant", content에 "4" 포함)
```

### Phase 4: Plan API 확인

```bash
# 11. Plan 목록 확인 (기존 대화가 있는 경우)
curl -s "http://127.0.0.1:19840/api/plans" \
  -H "Authorization: Bearer $TOKEN" | jq '.[0]'

# 12. Plan 상세 (ID를 알고 있는 경우)
curl -s "http://127.0.0.1:19840/api/plans/$PLAN_ID" \
  -H "Authorization: Bearer $TOKEN"

# 13. Plan 이벤트
curl -s "http://127.0.0.1:19840/api/plans/$PLAN_ID/events" \
  -H "Authorization: Bearer $TOKEN"
```

## 결과 보고 형식

각 테스트에 대해:

```
## Test N: [테스트 이름]
- **Command**: curl ...
- **Expected**: [기대한 결과]
- **Actual**: [실제 결과]
- **Status**: ✅ PASS / ❌ FAIL
- **Note**: [버그 발견 시 설명]
```

## 버그 발견 시

1. 보고만 하고 **코드를 수정하지 마세요**
2. 다음 형식으로 기록:

```
## Bug: [제목]
- **Endpoint**: GET/POST /api/...
- **Request**: curl 명령
- **Expected**: 기대 동작
- **Actual**: 실제 동작
- **Severity**: P0/P1/P2
```

## 참고 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/http_api.rs` | HTTP API 서버 (~430줄) |
| `src-tauri/src/lib.rs` | Tauri 앱 빌더 + HTTP API 시작 |
| `docs/ideas/httpApiTestInfraIdea.md` | API 설계 문서 |
| `docs/ideas/contextPackTieringIdea.md` | Tiering 설계 (8항목 완료) |

## 기술 스택 요약

| 계층 | 기술 |
|------|------|
| Desktop | Tauri 2 |
| Frontend | React 18 + TypeScript + Zustand 5 |
| Backend | Rust (Tauri commands + axum HTTP API) |
| DB | SQLite WAL (v30, vec_chunks 포함) |
| 벡터 검색 | sqlite-vec (HNSW, 384-dim cosine) |
| Agent CLI | Claude, Codex, Gemini, OpenCode, Ollama |

## DB 주요 테이블

| 테이블 | 핵심 필드 |
|--------|---------|
| projects | key, name, path, type, hidden |
| conversations | id, project_key, label, mode, usage_status |
| messages | id, conversation_id, role, content, engine, model, status, timestamp |
| plans | id, conversation_id, title, status, phase |
| plan_events | id, plan_id, event_type, actor, detail, created_at |
| artifacts | id, conversation_id, type, title, status |
| agent_jobs | id, conversation_id, engine, kind, status |

## 앱 실행 방법

```bash
cd /Users/d9ng/privateProject/tunaFlow
npm run tauri dev
# 콘솔에 "[startup] HTTP API token: ..." 출력 → 이 토큰 사용
```
