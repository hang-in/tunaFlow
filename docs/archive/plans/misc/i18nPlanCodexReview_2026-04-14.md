# i18nPlan Codex Review Memo

> 대상 문서: [i18nPlan](./i18nPlan.md)
> 작성일: 2026-04-14
> 목적: Opus 검토용 요약 메모

## 결론

현재 `i18nPlan.md`는 **실행 가능한 계획**이다.  
이전 리뷰에서 지적했던 문서 내부 모순은 해소됐고, 지금 단계에서 추가 설계 논쟁보다 `Phase 1` 착수가 우선이다.

다만 아래 2개는 구현 전에 명확히 고정하는 편이 안전하다.

1. `AppError -> stable error code`는 variant 이름 직렬화에 기대지 말고 명시 매핑으로 고정할 것
2. `error.json` 작성 전에 현재 `AppError` 인벤토리를 먼저 뽑아 누락 없는 코드 목록을 확정할 것

## 이번 수정에서 확인된 점

### 1. 기존 모순 해소

- `src-tauri/src/i18n/` 모듈은 만들지 않는 것으로 일관되게 정리됨
- `invoke` 공통 locale 인자는 계획에서 제거됨
- Rust 에러는 locale-aware 문자열이 아니라 `stable error code + context` 반환으로 정리됨
- `Phase 4`는 `4A / 4B`로 분리되어 일정과 리스크가 현실화됨
- `insightOrchestration` 영어 전환은 A/B 검증 필수로 격상됨

### 2. 현재 설계 판단

- UI i18n은 프론트 `react-i18next`로 처리하는 방향이 맞다
- 프롬프트 응답 언어는 기존 `user_profile.preferredLanguages` per-request 경로를 유지하는 것이 맞다
- Rust 쪽은 번역 책임을 갖지 않고 에러 코드를 반환하는 구조가 맞다
- `insightOrchestration`은 단순 번역 작업이 아니라 품질 변경이므로 별도 검증 단계가 필요하다

## 남은 구현 주의점

### A. AppError 코드 안정성

문서의 예시:

```json
{ "code": "not_found", "context": "branch" }
```

방향은 맞다. 다만 이 코드는 다음 성질을 가져야 한다.

- Rust enum variant rename과 무관해야 함
- 프론트 i18n 키와 1:1로 안정적으로 대응돼야 함
- 같은 의미의 에러가 호출 경로마다 다른 문자열로 새지 않아야 함

즉 `serde`로 variant 이름을 그대로 노출하는 방식이 아니라, 별도 code enum 또는 명시 매핑 함수로 고정하는 편이 맞다.

### B. AppError 인벤토리 선행

`error.json`을 만들기 전에 현재 `AppError` 계열과 프론트에 실제 노출되는 에러 경로를 먼저 한 번 정리하는 게 좋다.

이유:

- 누락된 코드가 있으면 fallback 문자열이 그대로 노출됨
- 동일 의미의 에러가 중복 키로 분산될 수 있음
- 이후 번역 파일 유지보수가 어려워짐

권장 순서:

1. 현재 `AppError` variant 목록 추출
2. 사용자 대면 에러만 선별
3. stable code 이름 확정
4. `error.json` 작성
5. 프론트 `extractErrorCode()` 경로 적용

## Opus 확인 요청 포인트

아래 3개만 확인하면 충분하다.

1. `AppError -> stable error code`를 별도 enum/mapper로 고정하는 방식에 동의하는가
2. `error.json` 작성 전 `AppError` 인벤토리 추출 단계를 Phase 4A 시작 조건으로 넣을지
3. `insightOrchestration` A/B 검증 기준이 현재 문서 수준으로 충분한지, 아니면 정량 기준을 더 넣을지

## Codex 최종 판단

이 계획은 이제 반대할 이유가 없다.  
남은 이슈는 제품 방향이 아니라 구현 디테일이다.  
Opus 검토가 끝나면 새 설계 논쟁 없이 바로 `Phase 1`로 들어가면 된다.
