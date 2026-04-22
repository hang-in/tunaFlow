# Roundtable Participant Surface Visibility Plan

상태: 중요 / P1
작성: 2026-03-30

## 배경

Roundtable backend와 생성 다이얼로그에는 이제 participant별로:

- `role`
- `blind`
- `max_tokens`(soft cap override)

를 표현할 수 있다.

하지만 현재 이 값들은:

- RT를 만들 때만 설정 가능하고
- 실행 중 Roundtable 화면이나 상태 표면에서는 다시 읽기 어렵다.

즉 blind verifier나 reviewer/verifier 분리가 실제로 들어갔더라도,
사용자가 현재 RT 구성이 어떻게 동작하는지 즉시 확인하기 어렵다.

## 목표

`RoundtableView`와 관련 runtime/status 표면에서 participant별:

- role
- blind 여부

를 최소 수준으로 다시 확인 가능하게 만든다.

핵심은 “설정 가능”에서 “실행 중에도 읽을 수 있음”으로 올리는 것이다.

## 왜 필요한가

### 1. blind verifier는 실행 표면에서도 보여야 한다

blind verifier는 일반 participant와 다르게:

- prior/current transcript를 보지 않고
- topic only 판단을 내린다.

이 차이는 RT 생성 후에도 명확히 보여야 한다.

### 2. role-based RT 설계가 실제로 드러나야 한다

이제 RT는 단순 multi-send가 아니라:

- proposer
- reviewer
- verifier
- synthesizer

같은 역할 분담을 전제로 한다.

그렇다면 실행 화면에서도 이 역할 구분이 보여야 한다.

### 3. 설정만 있고 실행 표면이 없으면 운용이 어렵다

RT를 다시 열거나,
과거 RT를 검토하거나,
현재 어떤 participant가 verifier인지 빠르게 확인할 때
실행 표면 정보가 필요하다.

## 이번 단계에서 할 것

### 1. RoundtableView participant 표면 보강

participant chip, header, row, 또는 status line 어디든:

- role badge
- blind badge/icon

를 최소 수준으로 표시한다.

과도한 메타 패널은 만들지 않는다.

### 2. 진행 상태 이벤트 표면에서 blind 식별

이미 status 이벤트에 `blind`가 포함되므로,
프론트에서:

- blind participant 실행 중임
- blind participant 결과임

을 구분할 수 있게 한다.

### 3. role은 읽기 쉬운 짧은 label로 보인다

예:

- `Prop`
- `Rev`
- `Ver`
- `Synth`

같은 짧은 배지 또는 pill이면 충분하다.

### 4. blind는 시각적으로 구분한다

권장:

- shield 아이콘
- 또는 `Blind` badge

일반 participant와 쉽게 구분돼야 한다.

## 이번 단계에서 하지 않을 것

- RT 생성 다이얼로그 재설계
- hard token cap enforcement
- verifier scoring
- lead decomposition
- RT 결과 judge 시스템

## 구현 원칙

- 읽기 쉬운 최소 표시
- 메타 정보는 짧고 스캔 가능해야 한다
- 기존 RT 레이아웃을 크게 흔들지 말 것
- role/blind를 확인하는 것이 목적이지 설정 패널을 또 만드는 것이 아님

## 성공 기준

- RT 실행/조회 화면에서 participant별 role을 다시 확인할 수 있다
- blind verifier 여부를 실행 중에도 식별할 수 있다
- 상태 이벤트/결과 표면 중 최소 한 곳 이상에서 blind가 명확히 보인다
- 기존 RT 흐름을 복잡하게 만들지 않는다

## 후속

이 단계 다음은:

1. soft cap/max token override UI 최소 노출 보강
2. role/blind/max token preset 저장 polish
3. 필요 시 verifier-focused RT preset

순으로 이어진다.
