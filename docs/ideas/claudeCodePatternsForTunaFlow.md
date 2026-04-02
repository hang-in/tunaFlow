# Claude Code 패턴 — tunaFlow 적용 검토

> Status: idea
> Created: 2026-04-01
> 출처: Claude Code 가이드 (wikidocs.net/book/19104) 12개 꼭지 분석

---

## 개요

Claude Code의 아키텍처 패턴 중 tunaFlow 고도화에 적용 가치가 있는 항목을 정리한다. 각 항목에 tunaFlow 현재 상태, 적용 방안, 예상 효과를 기술한다.

---

## 1. Hooks 자동 품질검사

> 출처: wikidocs.net/335605, wikidocs.net/333424

### Claude Code 패턴

Hook = 도구 실행 전후에 강제로 실행되는 검증 단계. CLAUDE.md 지침(무시 가능)과 달리 **시스템 레벨에서 강제 실행**됨.

**Hook 이벤트 주기:**
- `PreToolUse` — 도구 실행 전 (차단 가능)
- `PostToolUse` — 도구 실행 후 (결과를 피드백으로 주입)
- `Stop` — 턴 완료 시 (재시작 가능)
- `SessionStart/End` — 세션 생명주기

**Hook 타입:**
- `command` — 셸 스크립트 실행
- `prompt` — LLM 기반 판단 (yes/no)
- `agent` — 서브에이전트로 다단계 검증
- `http` — 외부 웹훅 호출

**핵심 패턴 — PostToolUse 자동 수정 루프:**
```
파일 수정 → PostToolUse Hook 발동 → ESLint 실행 → 에러 출력 → Claude가 자동 수정
```

### tunaFlow 현재 상태

- 에이전트 응답 후 자동 검증 없음
- 마커 감지(`plan-proposal`, `impl-plan` 등)는 프론트엔드에서 수동 파싱
- Developer 구현 완료 후 lint/test 자동 실행 없음

### 적용 방안

**Phase 1: 워크플로우 Post-Hook 패턴**

에이전트 응답(`agent:completed` 이벤트) 후 자동 검증 단계:

```typescript
// agent:completed 이벤트 핸들러에서
if (plan.phase === "implementation" && hasImplComplete(message)) {
  // 1. run_project_tests 자동 실행
  const testResult = await invoke("run_project_tests", { projectPath });
  // 2. 테스트 실패 시 Developer에게 자동 피드백
  if (!testResult.success) {
    await sendThreadMessage(`테스트 실패:\n${testResult.output}\n수정하세요.`, devEngine);
  }
  // 3. 테스트 성공 시 Review RT 자동 시작 (또는 사용자 확인)
}
```

**Phase 2: Pre-Hook 패턴 — 마커 검증**

에이전트에게 전송 전 프롬프트 검증:
- Implementation phase에서 plan 범위 밖 요청 감지 → 경고
- Review phase에서 코드 수정 요청 감지 → 차단

### 예상 효과

- Developer 구현 → 테스트 → 수정 루프 자동화
- 역할 벗어남 방지 (Reviewer가 코드 수정하는 상황 등)

---

## 2. 서브에이전트와 에이전트 팀

> 출처: wikidocs.net/335612, wikidocs.net/333425

### Claude Code 패턴

**서브에이전트 정의:**
- `.claude/agents/` 디렉토리에 YAML 프론트매터 + Markdown으로 정의
- 도구 제한: `tools: Read, Grep, Glob, Bash` (읽기 전용 리뷰어)
- 모델 선택: `model: sonnet` (비용 절감)
- 격리: `isolation: worktree` (독립 git worktree)
- 메모리: `memory: project` (에이전트별 독립 메모리)

**권한 최소화 원칙:**
- 리뷰 에이전트: Read-only 도구만 → 코드 수정 불가
- 수정 에이전트: Write/Edit 추가
- 도구를 명시하지 않으면 전체 상속 → **항상 명시적으로 제한**

**에이전트 팀 (실험적):**
- 독립 Claude 인스턴스가 메일박스/태스크 리스트로 소통
- 3-5명 최적, 파일 소유권 분리 필수
- Anthropic 내부에서 16 병렬 에이전트로 10만줄 C 컴파일러 개발 사례

### tunaFlow 현재 상태

- `docs/agents/{architect,developer,reviewer}.md` 템플릿 존재 (이번 세션에서 추가)
- ContextPack Tier 0에 플랫폼 안내 주입
- 에이전트 간 소통은 Branch/RT를 통한 메시지 기반
- 도구 제한 없음 — 모든 에이전트가 동일 권한으로 CLI 실행

### 적용 방안

**도구 제한 적용:**

tunaFlow가 에이전트 CLI를 호출할 때 역할별 권한 플래그 전달:

| 역할 | Claude 플래그 | 효과 |
|------|-------------|------|
| Architect | `--permission-mode plan` | 읽기 전용, 코드 수정 불가 |
| Developer | `--permission-mode acceptEdits` | 편집 자동 승인 |
| Reviewer | `--permission-mode plan` | 읽기 전용 |

```rust
// agents/claude.rs — stream_run에서 역할별 플래그 적용
fn build_args(input: &RunInput, role: Option<&str>) -> Vec<String> {
    let mut args = vec!["--print".into(), "--output-format".into(), "stream-json".into()];
    match role {
        Some("architect") | Some("reviewer") => args.push("--permission-mode plan".into()),
        Some("developer") => args.push("--permission-mode acceptEdits".into()),
        _ => {}
    }
    args
}
```

**에이전트별 독립 메모리:**

현재 `conversation_memory`가 대화 단위. 에이전트별 메모리는:
- `docs/agents/architect-memory.md` — Architect의 프로젝트 이해도
- `docs/agents/developer-memory.md` — Developer의 코드 컨벤션 학습
- 에이전트 응답 후 자동 업데이트 (compressed memory와 유사)

### 예상 효과

- Architect가 실수로 코드를 수정하는 상황 시스템 레벨 방지
- Reviewer의 코드 수정 불가 → 리뷰 신뢰도 향상
- 에이전트별 메모리 → 장기적으로 역할 전문화

---

## 3. Git Worktree 병렬 세션

> 출처: wikidocs.net/335608

### Claude Code 패턴

- `claude -w feature-login` → 독립 git worktree에서 격리된 세션
- 변경 없으면 자동 정리, 커밋 있으면 보존
- 5개 로컬 + 5-10개 클라우드 = 10-15 병렬 인스턴스 패턴

### tunaFlow 현재 상태

- Branch 시스템으로 대화/컨텍스트 분기 지원
- `branches.git_branch` 필드 존재 (git branch 연동 준비)
- 실제 git worktree 연동은 미구현

### 적용 방안

**Implementation Branch를 실제 git worktree로 격리:**

```
Plan 승인 → Implementation Branch 생성
  → git worktree add .worktrees/impl-{plan-id} -b impl/{plan-title}
  → Developer에게 worktree 경로 전달
  → 구현 완료 후 git merge 또는 PR
```

이미 `gitSyncBranchModelPlan_2026-03-29.md`에 설계가 있으므로, 워크플로우 파이프라인과 연결하는 것이 핵심.

### 예상 효과

- Developer 구현이 메인 브랜치에 영향 없음
- 여러 plan을 병렬로 구현 가능
- Rework 시 worktree 폐기 후 재생성 → 깔끔한 상태

---

## 4. 권한 시스템

> 출처: wikidocs.net/333420

### Claude Code 패턴

**6단계 권한 모드:** Default, AcceptEdits, Plan, Auto, DontAsk, BypassPermissions

**Auto 모드 Classifier:**
- 별도 모델(Sonnet)이 각 행동의 위험도 평가
- 기본 허용: 프로젝트 디렉토리 내 파일 작업, 의존성 설치
- 기본 차단: `curl | bash`, 프로덕션 배포, force push, IAM 변경
- 커스터마이즈: `autoMode.allow`, `autoMode.soft_deny`

**샌드박싱:**
- macOS: Apple Seatbelt, Linux: bubblewrap
- 파일시스템 격리 + 네트워크 격리
- 샌드박스 내에서는 Bash 자동 승인 → 샌드박스가 안전 보장

**Deny-at-any-level 원칙:** allow와 deny가 동시 매칭되면 deny 우선

### tunaFlow 현재 상태

- CLI 에이전트에 권한 플래그를 전달하지 않음
- 모든 에이전트가 동일 권한
- 프로젝트 경로 기반 격리만 존재

### 적용 방안

**단기: CLI 플래그 기반 권한 전달**

에이전트 호출 시 `--permission-mode` 전달 (아이디어 2의 도구 제한과 연동):

```rust
// send_common.rs — prepare_engine_run에서 role 기반 플래그 결정
let permission_mode = match plan_phase {
    "implementation" => "acceptEdits",
    "review" => "plan",
    _ => "default",
};
```

**중기: Auto 모드 Classifier 개념 적용**

에이전트 응답에서 위험 패턴 감지:
- `rm -rf` 명령어 포함 → 사용자 확인 요청
- 프로덕션 배포 관련 내용 → 경고
- plan 범위 밖 파일 수정 → 경고

### 예상 효과

- 역할별 권한 강제로 사이드 이펙트 방지
- 실수로 인한 코드 삭제/배포 위험 감소

---

## 5. 창시자(Boris Cherny) 워크플로우

> 출처: wikidocs.net/333437

### 핵심 패턴

1. **Opus + Plan 모드로 시작** → 아키텍처 충분히 논의 → auto-accept로 전환해서 구현
2. **5+5 병렬 세션**: 5개 로컬 + 5-10개 클라우드 동시 실행
3. **검증 피드백 루프가 #1 핵심**: "Claude에게 작업을 검증할 수 있는 방법을 주면 최종 결과 품질이 2-3배 향상"
4. **에러 기반 CLAUDE.md 업데이트**: Claude가 잘못할 때마다 CLAUDE.md에 추가 → 다음부터 방지
5. **`/simplify` 커맨드**: 병렬 에이전트가 재사용/품질/효율성 동시 리뷰
6. **서브에이전트 분산**: "use subagents" 키워드로 병렬 처리 활성화

### tunaFlow 적용 포인트

| Boris 패턴 | tunaFlow 적용 |
|-----------|--------------|
| Plan 모드 → auto-accept | Architect(plan) → Developer(acceptEdits) 이미 설계 |
| 검증 피드백 루프 | `run_project_tests` 자동 실행 + 결과 주입 (아이디어 1과 연동) |
| 에러 기반 CLAUDE.md | 워크플로우에서 review fail 사유를 `docs/agents/*.md`에 자동 누적 |
| 병렬 세션 | Branch + worktree 격리로 병렬 plan 실행 |

### 가장 중요한 인사이트

> **"검증 피드백 루프를 주면 품질이 2-3배"** — tunaFlow의 Review phase에 테스트 자동 실행이 핵심

---

## 6. Skills 직접 개발

> 출처: wikidocs.net/335610

### Claude Code 패턴

- `.claude/skills/review/SKILL.md` — 프로젝트별 slash command
- YAML 프론트매터: `name`, `description`, `context: fork`, `disable-model-invocation`
- 동적 컨텍스트: `` !`gh pr diff --name-only` `` → 실행 결과를 프롬프트에 삽입
- `context: fork` — 격리된 컨텍스트에서 실행
- `user-invocable: false` — Claude가 자동 판단으로만 호출 (사용자 직접 호출 불가)

### tunaFlow 현재 상태

- `~/.tunaflow/skills/*/SKILL.md` 시스템 존재
- vendor별 스냅샷 발행 (`publish-skills.sh`)
- 키워드 매칭으로 관련 스킬만 ContextPack에 주입
- `activeSkills` persist

### 적용 방안

**워크플로우 전용 스킬:**

```yaml
# ~/.tunaflow/skills/tunaflow-architect/SKILL.md
---
name: tunaflow-architect
description: tunaFlow 워크플로우의 Architect 역할 수행 시 참조
---

## Plan Proposal 형식
<!-- tunaflow:plan-proposal --> 마커를 사용하여 제안...

## 질문 우선 원칙
코드 작성 전 반드시 요구사항 확인...
```

워크플로우 phase에 따라 자동 활성화:
- `phase === "drafting"` → `tunaflow-architect` 활성
- `phase === "implementation"` → `tunaflow-developer` 활성
- `phase === "review"` → `tunaflow-reviewer` 활성

### 예상 효과

- 기존 스킬 시스템 재활용으로 추가 인프라 불필요
- Phase 기반 자동 활성화로 사용자 수동 선택 불필요

---

## 7. TDD 워크플로우

> 출처: wikidocs.net/335603

### Claude Code 패턴

5단계 분리:
1. 프로젝트 설정
2. **Red**: 테스트 먼저 작성 ("구현은 아직 하지 마")
3. **Green**: 테스트 통과하는 최소 구현
4. Edge case 확장
5. **Refactor**: 기능 추가 없이 품질 개선만

핵심: **테스트와 구현을 동시에 요청하지 않음** → 단계 분리 강제

### tunaFlow 적용 포인트

워크플로우 Implementation phase에 TDD 옵션:

```
Plan 승인 → Developer에게 단계 분리 지시:
1. 먼저 plan의 각 subtask에 대한 테스트 작성
2. run_project_tests로 전부 실패 확인 (Red)
3. 구현 시작 승인
4. 구현 완료 후 테스트 전부 통과 확인 (Green)
```

Developer 프롬프트에 TDD 모드 플래그 추가:
```
당신은 Developer입니다. TDD 모드로 작업합니다.
1단계: 먼저 각 subtask의 테스트를 작성하세요 (구현하지 마세요)
2단계: (사용자 승인 후) 테스트를 통과하는 최소 구현을 작성하세요
```

### 예상 효과

- 테스트 커버리지 보장
- Developer가 plan 범위를 벗어나는 구현 방지 (테스트가 경계 역할)

---

## 8. 파이프라인 통합

> 출처: wikidocs.net/335615

### Claude Code 패턴

8단계 파이프라인:
1. 로컬 개발 (lint + type check + test)
2. 서브에이전트 코드 리뷰
3. 문서 동기화
4. Git Hooks (pre-commit 자동 검증)
5. 커밋 + PR 생성
6. CI 트리거
7. 자동 리뷰 (GitHub Action)
8. 머지

**Pre-commit Hook**: `set -e` + lint + type check + test → 하나라도 실패 시 커밋 차단

### tunaFlow 적용 포인트

현재 파이프라인: Chat → Plan → Approve → Impl → Review → Done

**추가 가능한 자동화 단계:**

| 단계 | 현재 | 추가 가능 |
|------|------|----------|
| Impl 완료 후 | 수동 "Review RT 시작" | 자동 lint + test → 실패 시 Developer에게 피드백 |
| Review 완료 후 | 수동 verdict 확인 | PR 자동 생성 + CI 연동 |
| 머지 후 | 없음 | Plan status → done 자동 전환 + 문서 동기화 |

### 예상 효과

- Implementation → Review 전환 시 자동 품질 게이트
- Review 통과 후 PR/머지 자동화로 end-to-end 파이프라인 완성

---

## 9. 고급 기능 — Context 관리

> 출처: wikidocs.net/333430

### Claude Code 패턴

- **Compaction**: `/compact` + 지시문으로 대화를 요약 압축. 시스템 프롬프트/CLAUDE.md/환경정보는 보존
- **지연 도구 스키마 로딩**: MCP 도구 스키마가 context 10% 초과 시 on-demand 로드
- **서브에이전트 컨텍스트 격리**: 최종 요약(~420 토큰)만 메인에 반환 → 토큰 절약
- **Effort 레벨**: low/medium/high/max → 프롬프트 복잡도에 따라 자동 조절 (`/effort auto`)
- **Bare 모드**: `--bare` → 훅/스킬/MCP 없이 최소 로드 (CI/CD용)

### tunaFlow 현재 상태

- `compressed_memory` — 12+ 메시지 시 오래된 메시지 구조화 요약
- `context_mode` — Lite/Standard/Full 자동 선택
- `context_budget_cap` — Settings에서 조정 가능

### 적용 방안

**Effort 레벨 개념 도입:**

현재 `context_mode`가 유사하지만, 에이전트 호출 시 모델 effort도 함께 조절:

| context_mode | 모델 설정 |
|-------------|----------|
| Lite | Sonnet + low effort (빠른 응답) |
| Standard | Opus/Sonnet + medium effort |
| Full | Opus + high effort + thinking |

**Bare 모드 — CI/자동화용:**

워크플로우 자동화(테스트 실행, lint)에서 에이전트 호출 시:
- skills/rawq/retrieval 등 불필요한 context 제거
- 최소 프롬프트만으로 빠른 실행

### 예상 효과

- 단순 질문에 불필요한 토큰 소비 감소
- 자동화 파이프라인의 실행 속도 향상

---

## 10. 보안과 프라이버시

> 출처: wikidocs.net/333436

### Claude Code 패턴

**다층 방어:**
1. 권한 기반 접근 제어 (Permission Rules)
2. 샌드박스 격리 (Seatbelt/bubblewrap)
3. 네트워크 격리 (도메인 화이트리스트)
4. 프롬프트 인젝션 방어 (위험 명령 블랙리스트)
5. 자격증명 암호화 (OS Keychain)
6. Fail-closed 매칭 (미분류 명령은 수동 승인)

**데이터 보존 정책:** Consumer 30일~5년, Enterprise ZDR(즉시 삭제)

**프롬프트 인젝션 방어:**
- 위험 명령 블랙리스트 (`curl`, `wget` 기본 차단)
- 컨텍스트 격리 (웹 fetch 결과는 별도 윈도우)
- 이전에 허용된 명령도 의심스러우면 재확인
- 자연어로 복잡한 bash 명령 설명

### tunaFlow 적용 포인트

**단기 — 프롬프트 인젝션 기본 방어:**

에이전트에게 전달되는 프롬프트에서 위험 패턴 감지:
- `<!-- tunaflow:` 마커를 사용자 입력에서 감지 → 경고 (마커 위조 방지)
- 에이전트 응답의 마커가 올바른 형식인지 검증 후 파싱

**중기 — 자격증명 관리:**

현재 API 키가 환경변수로 노출. Tauri의 secure storage를 활용한 암호화 저장 고려.

**장기 — 네트워크 격리:**

에이전트가 외부 URL에 접근하는 것을 제한하는 정책. 특히 `curl | bash` 패턴 차단.

### 예상 효과

- 마커 위조를 통한 워크플로우 조작 방지
- API 키 노출 위험 감소

---

## 우선순위 종합

| 순위 | 항목 | 출처 | 효과 | 난이도 |
|------|------|------|------|--------|
| **P0** | 검증 피드백 루프 (테스트 자동 실행 + Developer 피드백) | 창시자 워크플로우, Hooks | 품질 2-3배 향상 | 중간 |
| **P1** | 역할별 CLI 권한 플래그 전달 | 서브에이전트, 권한 시스템 | 사이드 이펙트 방지 | 낮음 |
| **P1** | 마커 위조 방지 (입력 검증) | 보안 | 워크플로우 무결성 | 낮음 |
| **P2** | Phase 기반 스킬 자동 활성화 | Skills 개발 | 수동 설정 제거 | 중간 |
| **P2** | TDD 모드 옵션 | TDD 워크플로우 | 테스트 커버리지 보장 | 중간 |
| **P2** | Git worktree 연동 | Worktree 병렬 | 코드 격리 | 높음 |
| **P3** | Effort 레벨 연동 | 고급 기능 | 토큰 절약 | 중간 |
| **P3** | 에이전트별 독립 메모리 | 서브에이전트 | 장기 전문화 | 높음 |
| **P3** | Review 후 PR 자동 생성 | 파이프라인 통합 | end-to-end 자동화 | 높음 |
| 후순위 | Auto 모드 Classifier | 권한 시스템 | 위험도 자동 평가 | 매우 높음 |

---

## 참고 자료

| 페이지 | 제목 | 핵심 내용 |
|--------|------|----------|
| 335605 | Hooks 자동 품질검사 | PreToolUse/PostToolUse 강제 검증 패턴 |
| 333424 | 훅 레퍼런스 | 22개 이벤트, 4가지 훅 타입, JSON I/O |
| 335612 | 서브에이전트와 에이전트 팀 | 도구 제한, worktree 격리, 독립 메모리 |
| 333425 | 서브에이전트 레퍼런스 | YAML 정의, 권한 모드, 메모리 스코프 |
| 335608 | Git Worktree 병렬세션 | 5+5 병렬 패턴, 자동 정리 |
| 333420 | 권한 시스템 | 6단계 모드, Auto Classifier, 샌드박싱 |
| 333437 | 창시자 워크플로우 | 검증 루프 #1, CLAUDE.md 에러 누적, 병렬 |
| 335610 | Skills 직접 개발 | YAML 프론트매터, 동적 컨텍스트, fork |
| 335603 | TDD 워크플로우 | Red-Green-Refactor 단계 분리 강제 |
| 335615 | 파이프라인 통합 | 8단계 파이프라인, pre-commit 자동 검증 |
| 333430 | 고급 기능 | Compaction, Effort 레벨, Bare 모드 |
| 333436 | 보안과 프라이버시 | 다층 방어, 프롬프트 인젝션, ZDR |
