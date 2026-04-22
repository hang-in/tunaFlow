# ContextPack Algorithm Phase 1 Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

`ContextPack 알고리즘 개선 계획`은 여러 후보를 정리했지만, 그중 일부는 지금 바로 적용하기에 리스크가 크다.

특히:
- Claude 기반 compression 경로를 규칙 기반으로 완전히 대체하는 것
- 섹션 전체 예산 배분을 큰 폭으로 바꾸는 것

은 체감 효과는 커도 품질 회귀 위험이 있다.

따라서 1차는 **저리스크 알고리즘 개선**만 먼저 적용하는 것이 맞다.

## 목표

현재 ContextPack 파이프라인을 크게 흔들지 않으면서,

1. 불필요한 중복을 줄이고
2. 쓸모 없는 포맷 토큰을 줄이며
3. rawq 섹션의 커버리지를 높이는

작은 개선을 먼저 적용한다.

## 이번 단계에서 할 것

### 1. Jaccard 기반 유사 턴/블록 접기

적용 대상:
- cross-session
- context summary 계열의 반복 블록

목표:
- 거의 같은 메시지/응답/요약이 연속되거나 반복될 때 접어서 표현

원칙:
- 의미 손실이 적은 반복만 접는다
- aggressive dedup 금지

### 2. 마크다운 포맷 경량화

적용 대상:
- compression 전후의 긴 텍스트 섹션

목표:
- `**bold**`, `*italic*`, 과도한 공백, 불필요한 backtick 같은 토큰 낭비를 줄인다

원칙:
- 읽기 어려워질 정도로 포맷을 깨지 않는다
- 코드 블록/의미 있는 구조는 유지

### 3. rawq import 블록 접기

적용 대상:
- rawq code snippet

목표:
- import-heavy 파일에서 예산을 덜 낭비하게 한다

원칙:
- 연속 import/use/from/require 구간만 접는다
- 본문 로직은 최대한 보존

### 4. rawq 다해상도 표현

적용 대상:
- rawq 결과 top-N

목표:
- 상위 결과는 full snippet
- 그 다음은 skeleton/signature
- 나머지는 one-line reference

으로 표현해 같은 예산에서 더 많은 파일을 커버한다.

원칙:
- full snippet이 필요한 상위 결과 1~2개는 그대로 유지
- coverage 확대가 목표이지, 모든 결과를 압축하는 것이 목표는 아니다

## 이번 단계에서 하지 않을 것

- Claude compression 경로 제거
- 동적 예산 배분 전면 교체
- KKT/entropy/PRISM 같은 큰 알고리즘 도입
- vector retrieval

## 먼저 확인할 곳

- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/compression.rs`
- `src-tauri/src/commands/agents_helpers/rawq.rs`
- `src-tauri/src/commands/agents_helpers/guardrail.rs`

## 성공 기준

- cross-session/context summary의 반복이 줄어든다
- 전체 프롬프트에서 불필요한 포맷 토큰이 감소한다
- rawq 섹션이 같은 예산으로 더 많은 관련 파일을 보여준다
- 현재 품질을 크게 해치지 않고 적용된다

## 후속

이 단계가 안정화되면 다음 후보는:

1. 동적 예산 배분
2. Claude compression fallback 축소 실험
3. conversation retrieval

순으로 본다.
