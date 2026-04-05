# 코드 품질 감사 결과 — 2026-04-05 (세션 13)

> updated_at: 2026-04-05
> canonical: true

---

## 요약

tunaInsight 멀티 에이전트 분석(시니어 개발자 + QA 역할) + 사용자 검토 + 후속 리뷰 합의를 거쳐 도출된 코드 품질 감사 결과이다.
디자이너/기획자 보고서는 미제출 상태이며, 본 문서는 코드 품질 관점의 기술적 action item만 다룬다.

---

## 우선 실행 항목 (small, 5개)

| 순서 | 항목 | 공수 | 상태 |
|------|------|------|------|
| 1 | CSP 활성화 — `tauri.conf.json` CSP `null` → 기본 정책 설정 | small | ✅ 완료 |
| 2 | 빈 catch 정리 — 35개 → console.debug/warn + 라벨 (28파일) | small | ✅ 완료 |
| 3 | Non-null assertion 제거 — 프로덕션 11개 → 0개 (convId 별칭, optional chaining) | small | ✅ 완료 |
| 4 | CancelRegistry `parking_lot` 전환 — 6파일, poison-free Mutex | small | ✅ 완료 |
| 5 | 한국어 토큰 보정 — `estimate_tokens()` + CJK 6개 유니코드 범위 감지 + 6 tests | small | ✅ 완료 |

---

## Medium 우선순위 (후순위)

| 항목 | 공수 | 비고 |
|------|------|------|
| `AppError` → JSON 구조화 에러 응답 | medium | 프론트엔드에서 에러 유형별 분기 처리 가능하도록 |
| 이벤트 리스너 추상화 | medium~large | 안정화 후 진행. 현재 5엔진 10+ emit 지점이 개별 패턴 |
| 커버리지 활성화 | small | vitest + cargo-llvm-cov, CI 연동 시 함께 |

---

## 보류 / 불필요

| 항목 | 판단 근거 |
|------|-----------|
| Dynamic Import 정리 | 의도적 설계(lazy loading + 순환 의존성 방지). `as any` 4개만 개별 정리 대상 |
| Pre-commit Hook + TS 엄격성 | 안정화 단계 진입 후 적용. 현재는 빠른 반복이 우선 |
| Subprocess 환경 변수 격리 | `env_clear()` 시 에이전트 CLI 미동작. 로컬 전용 앱에서 보안 위협 낮아 과잉 조치 |
| Post-completion Rust 이동 | 방향에는 동의하나 현재 문제 사례가 적어 후순위. 실제 성능/안정성 이슈 발생 시 진행 |

---

## 보고서 누락 사항 (별도 분석 필요)

- **워크플로우 파이프라인 안정성**: 코드 품질보다 런타임 로직 정합성이 실사용 영향이 크다. Reviewer 프롬프트 정합성, verdict 스캔 타이밍, doom loop 감지 임계값 등은 코드 품질 감사 범위 밖이며 별도 검증이 필요하다.
- **엔진별 도구 차이**: 멀티 에이전트 오케스트레이션 앱에서 Claude/Codex/Gemini/OpenCode의 capability 차이(tool use 지원, 스트리밍 형식, 컨텍스트 한도 등)가 워크플로우 품질에 미치는 영향 분석이 필요하다.

---

## 참고

원본 보고서는 tunaInsight에서 시니어 개발자 + QA 역할 에이전트가 작성하였다. 디자이너/기획자 보고서는 미제출 상태이다.
