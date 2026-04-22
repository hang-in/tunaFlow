# bge-m3 최적화 — INT8 양자화 + CoreML/Metal EP

> Status: idea
> Created: 2026-04-22
> Trigger: 베타 전 reindex 실측 결과 (438 파일 / ~55분, CPU 200%) → 최적화 필요성
> 현재 구현: `src-tauri/src/agents/embedder.rs` (FP32 + CPU, intra=2/inter=1, 세마포어, yield)

---

## 1. 현재 상태

| 항목 | 값 |
|---|---|
| 모델 | BAAI/bge-m3 (1024-dim) |
| 정밀도 | FP32 |
| 런타임 | ONNX Runtime (`ort` crate) |
| 실행 장치 | CPU |
| 세션 풀 | 2 sessions × intra=2 threads × inter=1 |
| 실측 속도 | 약 7초/file (debug build) |
| 총 reindex 시간 | 438 파일 ≈ 55분 |

세션 35 수정으로 CPU 스파이크는 해결됐으나, 속도가 베타 사용자 온보딩 경험에 악영향.

---

## 2. 결론

**방향: INT8 먼저 → CoreML EP 다음**. 단 "품질 차이 미미" 는 **측정 전제 없이 받아들이면 안 됨**.

---

## 3. INT8 양자화

| 항목 | 효과 | 비고 |
|---|---|---|
| 디스크 | ~2.3GB → ~580MB | 초회 다운로드 UX 개선 |
| CPU 속도 | 2~4배 | Apple Silicon 특히 |
| 메모리 | ~4배 감소 | |
| 크로스플랫폼 | ✅ macOS/Linux/Windows 공통 | 플랫폼 분기 없음 |
| 리스크 | 임베딩 품질 1~3% 저하 | **측정 필요** |

**품질 검증이 핵심**:
- MTEB 벤치는 1~3% 저하 보고하나, tunaFlow 사용처(near-duplicate 제거, retrieval 순위, reranking) 에서 cos-sim 경계 근처 문서는 결과가 튈 수 있음.
- **내부 골든 쿼리셋** 10~20개로 Hit@5 / MRR 측정 후 결정.
  - 예: "ContextPack 구현 어디?", "bge-m3 CPU 이슈 수정 커밋", "PTY write queue FIFO"

**Rollout**:
1. FP32 default 유지 → INT8 opt-in flag 추가
2. 골든셋 통과 시 default 전환
3. 기존 사용자 DB 재인덱싱 필요 여부 판정 (embedding 공간 변화 크면 cross-version 호환성 부실)

---

## 4. CoreML / Metal EP

| 항목 | 이득 | 비용 |
|---|---|---|
| Apple Silicon ANE | 3~10배 추가 speedup | macOS 전용 |
| `ort` crate | CoreML EP feature flag 지원 | 번들 크기 증가 |
| 첫 실행 | — | **ANE 그래프 컴파일 30초~수 분** (첫 1회, 캐시됨) |
| ONNX op 커버리지 | — | bge-m3 일부 op 는 CPU fallback → 혼합 실행 |

**UX 함정**: 첫 실행 시 아무 로그 없이 1분간 멈추면 버그처럼 보임.
- "CoreML 컴파일 중..." 스피너 필수
- 캐시 키: `~/.tunaflow/models/bge-m3.coreml/<hash>/`

**Metal (MPS) vs CoreML (ANE)**:
- Apple Silicon 에서 transformer 추론은 **ANE 가 일반적으로 더 빠르고 전력 효율적**.
- Metal EP 는 Intel Mac 또는 discrete GPU 에 유리.
- bge-m3 같은 BERT-family 는 ANE 우선.

---

## 5. 주의할 점

### 세션 35 가드 유지
- CoreML EP 로 옮긴 뒤에도 **intra_threads=2, inter_threads=1, 세마포어, yield 유지**.
- ANE 는 시스템 공유 리소스 — 다른 앱 동시 사용 시 thermal throttle 가능.
- 가드 없으면 "앱이 가볍다가 갑자기 멈춤" 패턴 재발.

### 플랫폼 매트릭스
| 플랫폼 | 권장 조합 |
|---|---|
| macOS Apple Silicon | INT8 + CoreML EP |
| macOS Intel | INT8 + CPU (또는 Metal EP) |
| Linux/Windows | INT8 + CPU (+ CUDA EP 검토) |

→ **엔진 선택 로직 필요**: 런타임에 `ort::ExecutionProvider` 선택, 실패 시 CPU fallback. `ort` crate `with_execution_providers()` 로 구현.

---

## 6. 권장 action 순서

1. **INT8 모델 다운로드 옵션 추가** (크로스플랫폼, 리스크 최저).
2. **골든 쿼리셋 10-20개로 Hit@5 / MRR 측정**. 통과 시 default 후보.
3. **macOS 한정 CoreML EP feature flag** + 첫 컴파일 UX (스피너 + 캐시) 동시 설계.
4. **rawq daemon (snowflake-arctic-embed-s) 에도 같은 원칙 적용 검토** — 이쪽이 더 빈번히 호출됨.
5. **스레드 제한·세마포어는 건드리지 말 것** — 가속 후에도 safety net.

---

## 7. 측정 항목

| 지표 | 기준 | 목표 |
|---|---|---|
| Hit@5 (retrieval) | FP32 수준 | -1% 이내 |
| MRR@10 | FP32 수준 | -1% 이내 |
| 438 파일 reindex 시간 | 55분 (현재) | <15분 |
| 최대 CPU 점유율 | 200% | <80% |
| 디스크 모델 크기 | 2.3GB | <800MB |

**Phase 6 regression eval suite 에 retrieval quality 항목 추가 고려** — default 전환 전 회귀 방지.

---

## 8. 관련 문서

- `src-tauri/src/agents/embedder.rs` — 현재 구현
- `src-tauri/src/commands/document_index.rs` — 호출자
- 세션 35 (`project_session_2026-04-13_s35`) — CPU 스파이크 수정 맥락
- `docs/plans/promptRegressionEvalPlan.md` — Phase 6 eval 구조
- `docs/ideas/embeddingLatencyOptimizationIdea.md` (archive/ideas/completed/) — 이전 latency 검토
