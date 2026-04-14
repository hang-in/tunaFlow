---
title: Beta Release Readiness Plan
status: planned
created_at: 2026-04-14
related: cicdReleasePlan.md, skillSelectorAgentPlan.md
---

# 베타 배포 준비 계획

> 2026-04-14 세션에서 발견된 문제 및 결정사항 정리

---

## 1. 배포 전제

- **대상 플랫폼**: macOS only (arm64 + x86_64). Windows는 이후 포크 → 별도 릴리즈
- **에이전트 전제**: claude / codex / gemini / openai 중 1개 이상 설치된 사용자 대상
- **배포 방식**: GitHub Releases에 빌드된 `.dmg` 업로드 → `install.sh` 한 줄 설치
- **코드 서명**: Apple Developer Program 없음 → ad-hoc + `xattr -cr` 우회 안내
- **e2e 검증**: 사용자가 직접 샌드박스에서 빌드 버전 실행 예정

---

## 2. 발견된 문제 및 결정

### 🔴 큰 문제

#### 2.1 앱 아이콘 경로 미연결
- **현상**: `tauri.conf.json`의 `"icon": []`가 비어있음
- **실제 상태**: 참치 아이콘(`public/icon.icns`, `public/icon.ico` 등) 존재하나 config에 미연결
- **결정**: `tauri.conf.json`에 아이콘 경로 연결
- **작업**: `tauri.conf.json` → `"icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]` 설정

#### 2.2 macOS 코드 서명 불가
- **현상**: Apple Developer Program 없음 → Gatekeeper 차단
- **결정**: ad-hoc 서명으로 빌드, 설치 시 자동으로 `xattr -cr` 실행
- **작업**:
  - `install.sh`에서 설치 후 자동으로 `xattr -cr /Applications/tunaFlow.app` 실행
  - `INSTALL.md`에 Claude Code 등 에이전트가 실행할 수 있도록 명시

#### 2.3 rawq sidecar 번들 미포함
- **현상**: CI에서 placeholder만 생성, 실제 rawq 바이너리 없음 → 코드 검색/벡터 임베딩/Document RAG 전부 비활성
- **결정**: CI에서 각 플랫폼별 rawq 빌드 후 번들에 포함
- **작업**: `build.yml`에 `build-rawq` job 추가 (arm64/x86_64 각각 빌드)
- **참조**: `cicdReleasePlan.md` §3.3

#### 2.4 에이전트 CLI 미설치 시 동작 불가
- **결정**: 에이전트 1개 이상 설치를 전제로 배포. 단, 명확한 안내 제공
- **작업**:
  - `INSTALL.md`에 에이전트 설치 방법 명시 (claude / codex / gemini / openai codex)
  - 앱 내 진단 UI: 설치된 에이전트 감지 → 없으면 설치 안내 표시

#### 2.5 DB 마이그레이션 신규 설치 검증
- **결정**: 사용자가 직접 샌드박스 빌드 버전으로 검증 예정
- **체크포인트**: v1~v30 migration 순차 적용 + 앱 정상 시작 확인

---

### 🟡 사소한 문제

#### 2.6 버전 태그 동기화 수동
- **결정**: 자동화 필요
- **작업**: `build.yml`에서 태그(`v0.1.0`)를 파싱해 `tauri.conf.json` version 자동 주입
  ```yaml
  - name: Set version
    run: |
      VERSION=${GITHUB_REF_NAME#v}
      npx tauri build -- --config "{\"version\":\"$VERSION\"}"
  ```
  또는 `tauri-action`의 `--config` 옵션 활용

#### 2.7 HTTP API 포트 충돌 가능성
- **결정**: 잘 사용하지 않는 포트로 고정 변경 필요
- **작업**: 현재 포트 번호 확인 후 `47384` 등 충돌 가능성 낮은 번호로 교체
- **위치**: `src-tauri/src/http_api/mod.rs` 포트 설정 확인

#### 2.8 rawq 첫 실행 인덱싱 지연
- **결정**: 스플래시 스크린 도입 — 단, 빌드 테스트 후 필요 여부 결정
- **순서**: 빌드 테스트 → cold start 시간 측정 → 스플래시 도입 여부 결정
- **작업 (조건부)**:
  - rawq 인덱싱 진행률 표시 (기존 rawq status 이벤트 활용 가능)
  - 로고 스플래시: Tauri `splashscreen` 기능 또는 별도 로딩 윈도우

#### 2.9 DOOM 이스터에그
- **결정**: 보류 — 릴리즈 번들에 포함 유지, 제외 방법 추후 설계
- **현재**: `doom.html`, `public/emulators/` 번들에 포함 중

#### 2.10 Windows 지원
- **결정**: 첫 배포 macOS only → 이후 Windows 시스템에서 포크 후 별도 빌드/배포

---

## 3. 스킬 자동 적용 문제 (별도 개선)

> 이번 세션에서 발견. 세부 설계는 `skillSelectorAgentPlan.md` 참조

- **현상**: Layer C (프롬프트 키워드 매칭)이 `"store"`, `"서버"`, `"test"` 같은 일반 단어에도 스킬 주입 → 불필요한 Full context 모드 강제 발동
- **결정**: Layer C를 메타에이전트 기반으로 교체
  - 대화 첫 메시지 전송 시 Claude가 맥락 읽고 스킬 선택
  - `sessionSkills` (세션 단위 휘발성) 도입, `activeSkills` (수동/영속)와 분리
  - Layer C (`matchPromptToSkills`) 비활성화

---

## 4. README 정리 (완료)

- 세 가지 근본 문제의 **비율(41.8%, 36.9%, 21.3%) 및 가짜 출처 제거** — AI 생성 hallucination으로 확인
- 세 가지 문제 자체(맥락 붕괴/유령 위임/자기 검증 오류)는 실재하는 문제 — 설명 유지
- 참고문헌 0번(shalomeir.substack.com) 삭제
- DB v29→v30, 세션 이력 s15→s35, PTY/HTTP API/MCP 섹션 추가

---

## 5. 배포 전 체크리스트

### 🤖 Claude가 할 일 — 필수 (배포 불가 블로커)
- [x] `tauri.conf.json` 아이콘 경로 연결 — `icons/` 5개 경로 추가 완료
- [x] `.github/workflows/build.yml` 작성 — 2트랙(Lite/Full) 완료
- [x] `install.sh` 작성 — xattr 자동 실행, 트랙 선택(`--full` 플래그) 포함
- [x] `INSTALL.md` 작성 — 에이전트용, 에이전트 CLI 설치 안내 포함
- [x] HTTP API 포트 — 현재 `19840` (고유, 변경 불필요)
- [x] 버전 태그 자동화 — `build.yml`에서 `GITHUB_REF_NAME#v` 파싱 후 `--config` 주입

### 👤 사용자가 할 일 — 필수
- [ ] 샌드박스에서 빌드 버전 실행 + cold start 시간 측정
- [ ] 신규 DB(빈 상태) migration v1~v30 순차 적용 확인
- [ ] GitHub Secrets 설정 (`GITHUB_TOKEN` 외 필요 항목)
- [ ] 베타 태그 발행 (`git tag v0.1.0-beta.1 && git push --tags`)

### 🤖 Claude가 할 일 — 권장 (배포 후 빠르게)
- [ ] 앱 내 에이전트 진단 UI (설치된 CLI 감지 → 없으면 안내)
- [ ] rawq 인덱싱 진행률 표시 (스플래시 여부는 사용자 테스트 후 결정)
- [ ] 스킬 선택 메타에이전트 (`skillSelectorAgentPlan.md`)

### ⏸ 보류
- [ ] DOOM 이스터에그 릴리즈 제외 방법
- [ ] Windows 빌드 (macOS 배포 후)
- [ ] Apple 코드 서명 (유료, 정식 배포 시)
- [ ] 자동 업데이트 (`tauri-plugin-updater`)

---

## 6. 관련 문서

| 문서 | 내용 |
|------|------|
| `docs/plans/cicdReleasePlan.md` | GitHub Actions 빌드/릴리즈 workflow 설계 |
| `docs/plans/skillSelectorAgentPlan.md` | 스킬 자동 선택 메타에이전트 설계 |
| `docs/ideas/onboardingMetaAgentIdea.md` | 온보딩 메타에이전트 아이디어 (설치 포함) |
