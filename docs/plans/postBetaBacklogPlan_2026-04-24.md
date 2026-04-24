---
title: Post-beta backlog — 한계 섹션에서 뽑은 "해결 예정" 항목 모음
status: backlog (prioritization pending)
priority: P2 (베타 피드백 이후 재우선순위)
created_at: 2026-04-24
related:
  - docs/posts/07-rawq-code-review-graph.md
  - docs/posts/08-스킬-자동-적용.md
  - docs/posts/09-품질-보증-설계.md
  - docs/plans/sidecarPipelinePlan_2026-04-24.md
  - docs/plans/windowsBuildPlan_2026-04-24.md
  - docs/plans/metaAgentPlan.md
  - docs/plans/i18nCompletionPlan_2026-04-24.md
---

# Post-beta backlog

개발기 Wiki 10 편 + side 4 편 한계 섹션에서 **"해결 예정 (로드맵 있음)" 으로 분류된 항목** 을 한 곳에 모은 문서. 각 항목은 독립 plan 으로 분리하지 않고, 베타 피드백을 받은 뒤 우선순위를 재정렬한다. 그때 P1 로 올라온 것부터 개별 plan 으로 승격.

별도 plan 이 이미 있는 항목은 **본 문서에서 링크만 걸고 중복 서술하지 않는다.**

---

## 이미 plan 으로 분리된 항목

| 주제 | 전용 plan | 우선순위 | 출처 편 |
|---|---|---|---|
| rawq → CRG pipeline | [sidecarPipelinePlan_2026-04-24](./sidecarPipelinePlan_2026-04-24.md) | P1 | 7편 |
| Windows 빌드 지원 | [windowsBuildPlan_2026-04-24](./windowsBuildPlan_2026-04-24.md) | P1 | (README) |
| 메타에이전트 Phase 1-A/B | [metaAgentPlan](./metaAgentPlan.md) | P1 | 10편 |
| i18n A3-ext + PR B | [i18nCompletionPlan_2026-04-24](./i18nCompletionPlan_2026-04-24.md) | P2 | (README) |
| rawq upstream PR | ✅ 제출: [auyelbekov/rawq#11](https://github.com/auyelbekov/rawq/pull/11) | 완료 | 7편 |
| Manual verification gate (B-19) | [manualVerificationGatePlan_2026-04-24](./manualVerificationGatePlan_2026-04-24.md) | P1 (ready-to-implement, 피드백 반영 완료) | Issue #176 |

---

## 백로그 항목 (plan 미분리, 본 문서가 SSOT)

### B-1. Staleness 지표 표면화 (출처: 7편)

rawq 인덱스가 현재 파일 대비 얼마나 낡았는지를 ContextPack 섹션 상단에 "index built 5m ago · 3 files changed since" 형태로 노출.

- 기대 효과: 에이전트가 rawq 결과의 신뢰도를 조정 가능. 특히 직전에 파일 수정한 직후의 rawq hit 은 line 번호가 틀어질 위험 높음.
- 구현 부담: 낮음. rawq daemon 이 이미 watcher 로 dirty 파일 추적 중. ContextPack 섹션 빌더에서 이 정보 추출만 추가.
- 의존성: 없음.

### B-2. 한국어 토크나이저 도입 (출처: 7편)

현재 rawq 는 `rrf_weight = 0.9` 로 **의미 임베딩 위주 + BM25 거의 무시** 로 우회 중. 한국어 BM25 토크나이저 (mecab / khaiii 등) 를 rawq 에 붙이면 근본 해결.

- 구현 부담: 중간~높음. rawq upstream 에 추가 (로컬 patch 또는 별도 feature flag).
- 의존성: rawq upstream 관계 (PR #11 먼저 머지돼야 다음 PR 수월).

### B-3. CRG 언어 지원 확장 (출처: 7편)

현재 CRG 가 안정적으로 지원하는 언어는 Rust / TypeScript / Python 중심. 그 외 tree-sitter grammar 가 있는 언어 (Go, Java, C++ 등) 는 심볼 해석이 헐거움.

- 구현 부담: 높음. CRG upstream repo 수준 작업.
- 전략: upstream 이슈 제기 + 커뮤니티 참여. 직접 구현은 tunaFlow 스코프 외.

### B-4. Layer A 확장 — manifest 없는 프로젝트 힌트 파일 인식 (출처: 8편)

현재 `detect_project_stack()` 은 `package.json` / `Cargo.toml` 중심. `.tool-versions`, `mise.toml`, `asdf`, `.nvmrc`, `pyproject.toml` 등도 힌트로 활용.

- 구현 부담: 낮음. skillMappings 와 detect_project_stack 확장.

### B-5. Layer C 섹션 생략 마커 → tool-request 전환 (출처: 8편)

현재 "N sections omitted" 가 실측상 에이전트에게 의미 전달 실패. tool-request 형태로 바꿔 "omitted section 읽기" 도구 신설.

- 구현 부담: 중간. tool handler 신설 + 프롬프트 지시 업데이트.
- 의존성: tool-request 아키텍처 유지 (현재 marker 기반 운영 중, toolCallHandlerPlan 에 SDK function calling 대체 검토 있음).

### B-6. 로컬 커스텀 스킬 영속 — 멀티 루트 (출처: 8편)

현재 `publish` 가 `~/.tunaflow/skills/` 를 통째로 재생성. 사용자 커스텀 스킬이 날아감.

- 해결: system 경로 (snapshot) + user 경로 (보존) 분리. 런타임 합성.
- 구현 부담: 중간. publish 스크립트 + runtime loader 변경.

### B-7. 스냅샷 신선도 자동 감지 (출처: 8편)

`publish` 잊으면 runtime 이 stale. 자동 감지:
- 옵션 1: 앱 시작 시 `~/.tunaflow/skills/_snapshot.json` timestamp vs 원본 skill 소스 timestamp 비교
- 옵션 2: `skills-runtime-policy.md` 에 "publish 못 하면 경고 배너" UX 추가

- 구현 부담: 낮음.

### B-8. Fresh Session Rework (출처: 9편, 10편)

현재 Rework 는 같은 Developer 세션에 이어서 수행 → 실패 패턴이 context 에 누적 → 같은 판단 반복.

Optio 등 패턴 참고: Rework 마다 새 세션으로 시작 + 필요한 정보만 주입.

- 구현 부담: 높음. 세션 lifecycle + context 재조립 로직.
- 기대 효과: Doom Loop 발생률 감소 예상 (미검증).
- 선결: prompt 설계 (어떤 정보를 남길지) 필요.

### B-9. failure_lessons 정확도 개선 (출처: 9편)

현재 FTS5 + 파일 경로 매칭으로, 흔한 에러 키워드 매치 시 무관한 실패 10개 딸려옴.

해결 방향:
- 옵션 1: 임베딩 기반 의미 검색으로 교체 (비용 증가)
- 옵션 2: 매칭 스코어 cutoff 강화 + 상위 N 줄임
- 옵션 3: plan 단위 scope 좁히기 (현재 project_key 단위)

- 구현 부담: 낮음~중간.

### B-10. resolution 개별 추적 (출처: 9편)

Review pass 시 같은 plan 의 모든 unresolved failure_lessons 에 resolution 일괄 박음 → 거짓 귀인.

해결: finding 별 individual resolution 추적. subtask 연결이 있으면 그 subtask verdict 따르기.

- 구현 부담: 중간. DB 스키마 + workflow state machine 수정.

### B-11. session_links 자동 decay (출처: 5편)

FTS5 로 자동 발견한 cross-session link 가 한 번 붙으면 안 떨어짐. 주제 바뀐 뒤에도 score 유지.

- 해결: 접근 빈도 / recency 기반 decay. 주기적 cleanup.
- 구현 부담: 낮음. background worker 에 작업 추가.

### B-12. RRF 도입 재검토 (출처: 5편)

현재 FTS5 + vector **append** 방식. 프로젝트가 커져 chunk 수만 개가 되면 RRF 또는 cross-encoder re-ranker 가 나을 수 있음.

- 실측 트리거: 대형 프로젝트 한 번이라도 붙으면 재검토.
- 구현 부담: 중간.

### B-13. Cross-conversation 요약 캐시 (출처: 5편)

현재 retrieval 시마다 linked 대화 `conversation_memory` 를 직접 읽어 붙임. Layer 3.5 (프로젝트 레벨 요약 캐시) 도입 시 효율.

- 구현 부담: 중간. 캐시 invalidation 규칙 설계 중요.
- 참고: `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md` Phase 3.

### B-14. RT 라운드 자동 판정 게이트 (출처: 10편)

현재 라운드 완료 = 사용자가 수동 클릭. 에이전트 자동 판정 시도 시 자기 입장 강화 편향 발견.

- 해결 방향: "Human 판정 + AI 제안" 하이브리드. 에이전트가 근거와 함께 판정 제안, 사용자 1-click.
- 구현 부담: 중간. UX 설계 + synthesizer 프롬프트 조정.

### B-15. 결과 문서 품질 일관성 (출처: 10편)

Plan → Done 사이클의 결과 문서가 케이스마다 품질 차이 큼. Developer 출력 복붙 수준부터 정리된 결정 문서까지.

- 해결: 결과 문서 템플릿 강제 (마커 + required 섹션)
- 구현 부담: 낮음. persona 프롬프트 + marker schema 확장.

### B-16. 결과 문서 마커 잔존 (재발 방지) (출처: 10편)

한 번 고쳤는데 다른 경로 (insight_report auto-export) 에서 또 발견. 공통 sync 함수로 뽑지 않은 것이 원인.

- 해결: `syncResultReport` 같은 공통 유틸로 통일 + 모든 호출 경로가 거치도록.
- 구현 부담: 낮음.
- 긴급도: 재발 방지 차원에서 높음.

### B-17. 실전 테스트 자동화 — 하네스 확장 (출처: 10편)

s37 하네스를 확장해 공개 레퍼런스 프로젝트에 대해 야간 배치 풀사이클.

- 구현 부담: 높음. 비용 부담 큼.
- 우선순위: 낮음 (리소스 문제).

### B-18. 커스텀 OpenAI-compat 엔드포인트 등록 (출처: Issue #175 Extended)

MVP (`customEndpointConfigPlan_2026-04-24`, 머지: `6cc991c`) 는 Ollama / LM Studio URL override 만. Extended 범위는 임의 label 의 엔드포인트 (vLLM / Groq / Together AI / OpenRouter / Fireworks 등) 등록.

- 필요 요소: 엔진 dropdown 동적화 (ENGINE_CONFIGS 정적 → registry 기반), per-endpoint API key 보관 (keyring 활용), GET `{base}/v1/models` HTTP discovery, 엔진 추가/삭제 UX.
- 구현 부담: 중간~높음 (2~3일, UX 설계가 가장 큰 변수).
- 선결 조건: MVP 머지 ✅ + 베타 피드백 2~4주 (어느 엔드포인트가 실제 수요 높은지 확인).
- 의존성: MVP plan 완료.

### B-19. 사용자 확인 게이트 (Manual verification gate, 출처: Issue #176)

→ **상세 설계 plan**: [manualVerificationGatePlan_2026-04-24](./manualVerificationGatePlan_2026-04-24.md)

- 상태: `ready-to-implement` / P1 (2026-04-24 커뮤니티 피드백 반영 완료)
- 피드백 반영 결과: fail 사유 입력은 **optional**, 사유 미입력 시 rework_reason 에 "manual verification failed" placeholder.
- 요약: `impl-complete` 직후 `⚠️ Manual:` 라인 파싱 → UI dialog (pass/skip/fail + optional 사유) → fail 있으면 기존 Rework 경로, pass 면 Review 진행. Settings 에 skip 토글.

### B-20. Anthropic upstream 에 stream-json permission_request 이벤트 요청 (출처: Issue #178)

- 카테고리: **outreach** — upstream 기능 요청
- 우선순위: **P2** — 단기 차단 없음 (`--dangerously-skip-permissions` 플래그로 interim fix 완료, PR #178 대응 참조)
- 배경: Claude CLI 의 `stream-json` 프로토콜에 `permission_request` 이벤트가 부재 → tunaFlow 에서 per-tool 승인 UI 를 구현할 경로가 없음. 이슈 [#178](https://github.com/hang-in/tunaFlow/issues/178) 제보자(`batmania52`)의 기술 분석 완료.
- 필요 요소:
  1. Anthropic `claude-code` repo (또는 공식 채널) 에 기능 요청 이슈 제기
  2. tunaFlow 측 대체 UI 승인 흐름 설계 (PTY 복원 또는 stdin-over-WS 채널)
  3. 이벤트 채택 시 `--dangerously-skip-permissions` 되돌리고 per-tool 승인 UI 구현
- 구현 부담: outreach 자체는 낮음 / 채택 후 UI 설계는 중간
- 선결 조건: 현 hotfix (#178) 머지 ✅ / upstream 응답 대기

---

## 운영 정책

- 사용자 / 커뮤니티 피드백 받으면 해당 항목을 **P1 로 올림** + 별도 plan 파일로 승격
- 우선순위 변경 시 본 문서의 항목 앞에 `[P1]` 태그
- 본 문서는 **살아있는 문서** — 베타 이후 2~4 주마다 점검

## 우선순위 제안 (Architect 초안, 사용자 확정 대기)

1. **B-16 결과 문서 마커 잔존** — 재발 방지 중요, 구현 부담 낮음
2. **B-1 Staleness 지표** — 낮은 구현 부담 대비 효과 좋음
3. **B-15 결과 문서 품질 템플릿** — 베타 사용자 첫 인상에 영향
4. **B-7 스냅샷 신선도** — 간단, 효과 명확
5. **B-10 resolution 개별 추적** — DB 스키마 변경 요구, 베타 후 수월

그 외는 피드백 받아가며 조정.
