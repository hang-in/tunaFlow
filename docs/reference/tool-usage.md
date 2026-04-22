---
title: Development Tool Usage
updated_at: 2026-04-22
canonical: true
status: active
owner: tunaFlow-core
---

# Development Tool Usage

시스템에 설치된 고성능 도구들. 기본 도구(`find` / `grep` / `cat`) 대신 사용한다. CLAUDE.md 에서 분리 — **도구 사용 전에만** 읽는다.

## 1. 코드 검색 / 조작 (speedy-claude)

| 대신 | 사용 | 이유 |
|------|------|------|
| `find . -name` | `fd -e ts` | 64x 빠름, `.gitignore` 존중 |
| `grep -r` | `rg "pattern"` | SIMD 가속, 자동 멀티스레드 |
| `sed -i` | `sd 'old' 'new'` | BSD/GNU 차이 없음, 12x 빠름 |
| `cat file` | `bat file` | 구문 강조, 줄 번호 |
| `diff a b` | `difft a b` | AST 기반 구조 비교 |
| `ls` | `eza -la` | 아이콘, 색상, git 상태 |

## 2. 멀티 파일 치환

- **단순**: `fd -e ts | xargs sd 'old' 'new'` (1 커맨드)
- **대화형**: `ambr 'old' 'new'`
- **Read + Edit 루프 금지** — 한 번의 커맨드로 일괄 처리

## 3. 프로젝트 분석 도구

| 도구 | 명령 | 용도 |
|------|------|------|
| **rawq** (v0.1.1) | `rawq search "키워드"` | 코드 시맨틱 검색 (임베딩 기반 하이브리드) |
| | `rawq map .` | AST 기반 코드베이스 구조 출력 |
| | `rawq daemon status` | daemon 상태 확인 |
| **code-review-graph** (v2.3.1) | `code-review-graph status` | 그래프 통계 (노드/엣지/파일) |
| | `code-review-graph detect-changes` | 변경 영향 분석 + risk score |
| | `code-review-graph update` | 증분 인덱스 업데이트 (변경 파일만) |
| **context-hub** | `chub search "react hooks"` | 라이브러리/프레임워크 문서 검색 |

## 4. 사용 시 주의

- **rawq daemon 이 꺼져있으면** 첫 검색이 수분 걸림 → `rawq daemon start --background` 먼저.
- **rawq / CRG 인덱스가 오래되면** 결과 부정확 → 대규모 리팩토링 후 `rawq index build` + `code-review-graph build` 재실행.
- **CRG `detect-changes`** 는 git diff 기반 → commit 되지 않은 변경도 감지.
