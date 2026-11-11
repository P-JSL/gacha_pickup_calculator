# 4. gacha_pickup_calculator — Rust 확률·환급 계산기

**저장소:** [P-JSL/gacha_pickup_calculator](https://github.com/P-JSL/gacha_pickup_calculator)  
**소스 분석 기준:** `main` / [`eedc576b2d67e04976ecf475341721a4dcce1a2b`](https://github.com/P-JSL/gacha_pickup_calculator/commit/eedc576b2d67e04976ecf475341721a4dcce1a2b)  
**보고서 작성일:** 2026-11-11 (KST)  
**대상:** 개발자 · 운영자 · 릴리스 담당자  
**개선 후보:** 4개 (우선순위·수정 위치·구현·검증·원복 기준 포함)  
**상태:** 분석·구현 제안만 작성. 실제 소스 최적화·동적 성능 측정·프로그램 배포는 수행하지 않음.  
> 문서 내 장·절 번호는 원래 6개 저장소 통합 보고서의 번호를 보존한 것입니다. 코드 근거 링크는 작성 시점의 커밋으로 고정했습니다.

## 4.1 책임과 실행 구조

**주 역할:** 붕괴: 스타레일, 명조, 스텔라 소라, 이환의 캐릭터·전용 장비 목표 획득에 필요한 자원 및 환급 반영 성공률을 계산합니다. GUI와 계산 도메인을 분리하고, 입력 완료 후 사용자가 명시적으로 계산을 실행합니다.

```text
src/main.rs (실행 및 로거 초기화)
  → app/lifecycle.rs (egui 화면·Run Simulation)
  → app/controller_session/calculation.rs (입력 스냅샷, 세대번호, 비동기 계산 요청)
  → app/compute.rs (CPU 작업 예산·계산 경로 배분)
     ├─ probability/banners.rs · distributions.rs (정확한 기초 확률 분포/공유 자원)
     ├─ domain/feedback/config.rs · rolls.rs · runner.rs (게임별 환급 시뮬레이션)
     └─ domain/feedback/results.rs (결과 CDF·분위수·성공 조건 요약)
  → app/funding.rs (자원·환급 후 부족분 및 목표 달성 확률)
  → ui/dashboard/overview.rs · results.rs (최종 대시보드)
  → app/session_log.rs (세션 기록)
```

진입점은 [src/main.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/main.rs#L31)입니다. `SessionLogger::start_default()`가 실행 때 초기화되고, [src/app/lifecycle.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/lifecycle.rs#L128)의 **Run Simulation** 버튼이 `recalculate()`를 호출합니다. 입력 변경은 [src/app/controller_session/calculation.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/controller_session/calculation.rs#L5)의 `invalidate_calculation()` 경로에서 기존 결과를 무효화하고 **자동 재실행하지 않습니다**. 계산 요청은 세대번호와 구성 스냅샷을 사용하며 오래된 응답을 버리도록 설계되어 있습니다.

## 4.2 알고리즘 경계 — 개발자가 반드시 구분할 것

| 경로 | 실제 역할 | 정확성/검증 관점 |
|---|---|---|
| [src/probability/banners.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/probability/banners.rs#L115) | 게임별 천장·픽업 상태 전이와 첫 목표 달성 분포 | 상태 전이 합계 1, 확정 천장 및 목표 보유 수 검증 |
| [src/probability/distributions.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/probability/distributions.rs#L76) | 캐릭터·무기에 공유 티켓을 적응적으로 배분하는 성공률 | 단독 목표 확률 합산으로 공동 달성률을 대체하지 말 것 |
| [src/domain/feedback/rolls.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/rolls.rs#L82) | 각 게임 4성·5성 판정 및 재화 환급 | 획득 순서, 중복 돌파, 환급 규칙, 확정 우선순위 |
| [src/domain/feedback/runner.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/runner.rs#L47) | 시도 인덱스별 난수 시드 및 병렬 경로 샘플링 | 같은 입력·샘플 수의 재현성, CPU 작업 분할 독립성 |
| [src/domain/feedback/results.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/results.rs#L113) | 보유권만 성공 / 환급 추가 성공 / 실패를 서로 배타적으로 분류 | 세 성공 단계 합, 표본 CDF 오차·분위수 구분 |
| [src/app/funding.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/funding.rs#L85) | 티켓·재화 환산 및 목표 필요 추가 자원 | 이중 차감·이중 환급 금지, 잔여 재화 소수 단위 보존 |

**핵심:** 환급 미반영 분포의 수학적 계산과, 환급을 실제로 재사용해 성공 여부를 추정하는 **Monte Carlo 계산은 별개**입니다. 최종 성공률에는 `feedback`이 있으면 환급 포함 경로를 사용합니다. 단순히 `보유권 + 평균 환급권`을 예산으로 넣어 원래 분포를 조회하면 시뮬레이션과 같은 확률이 된다는 보장은 없습니다. 환급 획득의 **시점·게임 규칙·경로 의존성**이 있기 때문입니다.

[src/app/compute.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/compute.rs#L38)은 CPU 가용성을 확인하되 계산 예산을 **1~8**로 제한하며, 명조의 추가 전략 계산과 환급 경로를 함께 조율합니다. [src/domain/feedback/config.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/config.rs#L53)는 게임별 `FeedbackConfig`를 생성하고 4성 캐릭터 최대 돌파 보유 비율의 기본값을 `1.0`으로 둡니다. 일부 비픽업 확률 구성은 구현 주석에 명시된 모델 근사입니다.

## 4.3 데이터·이미지·갱신·로그

- **카탈로그:** [src/data/catalog.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/catalog.rs#L108)이 포함된 카탈로그를 파싱하고, 캐릭터와 전용 장비를 식별자 기반으로 연결합니다. 목록 순서나 이름 유사성만으로 매핑하면 안 됩니다.
- **원격 카탈로그 갱신:** [src/data/runtime_updates.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/runtime_updates.rs#L134)에서 GitHub의 브랜치 SHA와 커밋 서명을 확인하고, 고정 커밋의 허용된 원본 자료를 내려받아 검증된 묶음으로 게시합니다. 실행 파일 자체는 원격 콘텐츠로 교체하지 않습니다. [src/data/runtime_updates/network.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/runtime_updates/network.rs#L11)은 URL 및 응답 크기·HTTPS 조건을 제한합니다.
- **이미지 무결성:** [src/data/security.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/security.rs#L81)은 내부 SHA-256 매니페스트와 파일 바이트를 대조합니다. 크기·경로 검증도 별도로 존재합니다.
- **로그:** [src/app/session_log.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/session_log.rs#L47)에서 실행 파일이 위치한 폴더 기준 `log/session.log`를 시작 시 새로 작성합니다. 프로그램 내 로그 뷰어는 최근 **20,000줄**을 표시하며 파일 기록과 독립적입니다. 실행 파일 폴더에 쓸 수 없으면 다른 경로/메모리로 대체될 수 있으므로 UI에서 실제 로그 경로와 경고를 확인해야 합니다.

### 운영자 조작 절차

1. **실행 전:** Windows x64용 v1.0.18 패키지의 ZIP·EXE·이미지 디렉터리가 함께 있는지 확인합니다. 기존 로그 보존이 필요하면 **실행 전에 `log/session.log`를 백업**합니다(재실행 시 기존 세션 기록 초기화).
2. **계산 전:** 게임, 캐릭터·장비, 목표 돌파/재련, 보유 뽑기권, 공유·전용 재화, 천장, 픽업 확정, 환급 적용 여부, 표본 수를 화면에서 기록합니다.
3. **실행:** 모든 입력을 끝내고 **Run Simulation**을 누릅니다. 계산 중 입력을 바꾼 경우에는 다시 실행해야 합니다.
4. **확인:** 대시보드 최상단 **두 목표 최종 공동 달성 확률**, 환급 전/후 비교, 보유권 성공/환급 후 추가 성공/실패(지원 게임), 신뢰/추정오차 설명을 확인합니다.
5. **장애:** 로그 화면에서 `CALC_REQUEST` → `CALC_REFUND_START/DONE` → `CALC_RESULT` 또는 `CALC_RESULT_ERROR`를 확인합니다. 동일 입력·표본 수로 재현하되 `session.log`를 백업하지 않고 재시작하면 기존 로그가 삭제될 수 있습니다.

### 개발자 검증 체크리스트

```sh
cargo test --locked
cargo test --locked --test v1_contracts
cargo test --locked --test performance_regression
cargo clippy --locked --all-targets -- -D warnings
```

테스트 파일 [tests/v1_contracts.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/tests/v1_contracts.rs#L9)은 확률분포 정규화·천장 경계·재화 반올림·단조성 등을 확인합니다. 환급 계산 로직을 변경할 때는 최소한 **기초 분포 ↔ 환급 경로 구분, 게임별 중복 환급, CPU 1/8 경로 동일성, 고정 시드, 입력 변경 후 무효화, 공동 목표 예산 중복 계산**을 회귀 검증해야 합니다. 위 명령은 제안된 테스트이며 본 보고서 작성 중 실행하지 않았습니다.

**변경 위험:** `rolls.rs`의 4/5성 판정과 `feedback/results.rs`의 CDF 기준을 동시에 바꾸면 계산 결과 표시가 기존과 달라질 수 있습니다. UI 문구만 손보더라도 `available_shared`와 `raw_shared/feedback`의 X축(티켓 단위)을 일치시키는지 확인하세요.

---

## 4.4 성능·효율 최적화 심층 검토 (추가)

**성능 성격:** CPU 중심 확률 계산 + 메모리 내 표본 집계 + egui 렌더링. **LLM 추론 엔진은 없습니다.** 게임 확률 모델은 확률분포/시뮬레이션이며 생성형 언어 모델이 아니므로 'LLM 양자화·토큰 생성 속도' 항목은 해당하지 않습니다.

### 4.4.1 이미 적용된 최적화 — 중복 제안하지 말 것

| 구현 | 소스 근거 | 의미 및 남은 한계 |
|---|---|---|
| 정확한 첫 성공 분포, CDF, 이진 탐색 분위수 | [distributions.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/probability/distributions.rs#L23) | CDF 조회가 이미 단순 배열 접근·`partition_point`입니다. 선형 분위수 탐색으로 되돌리면 안 됩니다. |
| 조밀한 분포 FFT 합성 | [fft.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/probability/fft.rs#L47) | 비용이 작은 희소 입력에는 직접 합성을 유지하고, 적합한 조밀 입력에 FFT를 사용합니다. FFT 전면 적용은 정확도·비용상 부적합합니다. |
| 표본 번호 기반 고정 난수 시드 | [runner.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/runner.rs#L47) | 병렬 작업 순서가 바뀌어도 동일 표본 번호는 같은 난수 경로를 갖습니다. |
| 제한된 작업자와 명조 3개 전략 병렬 예산 | [compute.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/compute.rs#L38) | CPU 예산을 1~8로 제한해 과도한 중첩 병렬화를 억제합니다. 단순히 스레드를 늘려 개선할 수 있다고 볼 근거가 없습니다. |
| 시뮬레이션 버튼에 의한 명시적 계산·세대번호 폐기 | [calculation.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/controller_session/calculation.rs#L17) | 입력마다 재계산하지 않으며 늦게 도착한 과거 결과를 폐기합니다. 배경 작업 자체의 조기 중단 가능성은 별도 측정이 필요합니다. |
| 이미지 비동기 디코딩/검증 후 텍스처 캐시 | [catalog/ui.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/catalog/ui.rs#L15) | UI 매 프레임 고화질 리사이즈하지 않습니다. 텍스처 캐시가 장시간 사용 중 커지는지 별도 계측하는 것이 우선입니다. |

### 4.4.2 추가 최적화 후보와 위험

| ID/우선 | 코드·현 상태 | 최적화 가설 | 반드시 지킬 계약·선행 조건 |
|---|---|---|---|
| **G-P1** | 환급 표본 `scenarios` 전체 보유, 표본별 결과를 `sort_unstable_by_key`, 정렬된 `budgets`와 `raw_pulls`도 보유. [runner.rs L167-L215](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/runner.rs#L167) | 표본 수 10만/100만 등에서 정렬과 세 벡터 보관의 CPU·메모리 비중을 계측. 표본 구조 패킹, 예산별 히스토그램 또는 키 정렬 방식 비교. | `scenario_at_quantile`, 조건부 평균, Wilson/DKW, `success_breakdown`에 개별 표본 정보가 필요합니다. 요약만 남기는 변경은 현재 API/결과를 깨므로 금지. |
| **G-P1** | 명조 환급 전략 3종을 별도 계산하여 보유. [compute.rs L80-L116](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/compute.rs#L80) | 사용자가 실제로 요청/표시한 전략과 표본 수에 따라 계산 우선순위를 조정하거나 지연 계산할 수 있는지 평가. | 현재 모든 전략을 동시 비교하는 UI 출력·동일 시드 계약을 유지해야 합니다. 지연 계산은 화면 응답 의미의 변경이므로 먼저 UX·정확성 테스트 필요. |
| **G-P2** | 환급 결과에서 특정 예산별 `success_breakdown`/`successful_means`는 표본 스캔을 사용. [results.rs L112-L137](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/results.rs#L112), [L199](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/results.rs#L199) | 하나의 계산 결과에 대해 여러 예산값을 반복 질의한다면 누적 합계(prefix sums) 또는 현재 예산 결과 캐시 검토. | 표본 정렬 경계와 0표본/0성공/동점 처리, 소수점 연산 순서 유지. 우선 재호출 횟수 계측 후 도입. |
| **G-P2** | 차트는 CDF를 샘플링해 화면 그래프를 생성. [charts.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/ui/charts.rs#L146) | 창 크기·결과 세대가 바뀌지 않은 프레임에서 좌표/레이아웃 갱신 비용 측정; 필요한 경우 세대번호+가용폭 기준 시각화 캐시. | 마우스오버 정확값, 표시 확률과 시뮬레이션 최종값의 동일성, 화면 크기 적응성 유지. |
| **G-P3** | 릴리스 이미지가 디코딩 후 작은 아이콘을 112px까지 보간. [catalog/ui.rs L87-L105](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/catalog/ui.rs#L87) | 미리보기 크기별 텍스처 캐시의 상한·해제 동작을 측정하고, 필요하면 화면 전환 시 오래된 비사용 텍스처 정리. | SHA-256 매니페스트 일치, 오프라인 이미지 탐색, 선택 전용 장비 아이콘의 정확한 대응 유지. |

**핵심 병목 후보:** Monte Carlo 표본 생성 자체와 정렬·표본 보관. 다만 현 코드에는 이미 불필요한 중복 정렬 제거가 구현돼 있습니다([runner.rs L210-L214](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/runner.rs#L210)). 따라서 '정렬 제거'가 아니라 **구체적 프로파일에 따른 자료구조 개선**이 정확한 제안입니다.

### 4.4.3 실측 방법·합격 기준

```sh
cargo test --locked --all-targets
cargo test --locked --release --test performance_regression -- --ignored --nocapture
cargo test --locked --test v1_contracts
cargo clippy --locked --all-targets -- -D warnings
```

고정된 **게임·천장·보유권·환급 설정·표본 수**에서 CPU 1개/8개 경로를 비교하고 `total time`, Monte Carlo sample/s, 정렬 단계 시간, 최고 RSS, GUI 프레임 지연(p95), 취소 후 작업 종료 지연을 기록합니다. 워밍업 후 최소 5회 반복하여 중앙값과 범위를 함께 제시합니다. `performance_regression.rs`의 시간 출력은 **참고값**이며 성능에 대한 자동 합격 임계값은 아닙니다.

**필수 회귀:** 같은 표본 인덱스의 원시 결과 및 정렬된 표본/분포 일치, 원래 예산의 성공률과 단계별 성공 비율 합산, 0~100% 확률 경계, FFT↔직접 합성 오차, 게임 4종의 천장과 환급 정확성. **반려:** 보유권·환급 포함 확률의 뜻이 바뀌거나 더 높은 속도 때문에 표본 편향·누락이 발생하는 경우. 측정 전 '몇 배 빨라짐'을 주장하지 않습니다.

## 4.5 확률 계산기: 구체적인 코드 개선 설계 (추가)

**진입 기준:** [compute.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/compute.rs#L33)에서 요청이 시작되고, [runner.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/runner.rs#L153)에서 각 시뮬레이션 결과가 정렬·요약되며, [results.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/results.rs#L112)와 [charts.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/ui/charts.rs#L146)가 소비합니다. 현재 FFT와 CDF 이진 탐색, 표본 번호별 난수 시드는 이미 최적화되어 있으므로 되돌리지 않습니다.

### IMP-G01 · Monte Carlo 표본 정렬·메모리 계측 후 자료구조 변경 (P1)

- **위치:** `src/domain/feedback/runner.rs`의 `simulate_chunk()`, 시나리오 벡터 예약, `sort_unstable_by_key()`, `budgets`·`raw_pulls` 생성. [근거 L167-L220](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/runner.rs#L167).
- **구현 순서:** ① 표본 생성/스레드 결합/정렬/요약 구간의 `Instant` 측정값을 개발자용 벤치마크에서 따로 출력합니다. ② `10,000/100,000/1,000,000` 표본에서 `size_of::<RefundScenario>()`, 세 벡터 실제 capacity, Peak RSS, sort 시간 및 샘플/s를 기록합니다. ③ 정렬이 실제 병목인 경우에만 `scenario.initial_flexible_required`의 단일 키 정렬 방식과 오프셋 정렬(`Vec<usize>` 인덱스)을 같은 표본에서 비교합니다. ④ 메모리가 병목이면 외부 API에 영향 없는 **내부 저장형 표현**만 대상으로 패킹/필드 범위 축소 가능성을 검토합니다.
- **피해야 할 변경:** `budgets`만 남기고 `scenarios`를 버리면 `scenario_at_quantile()`, 조건부 평균 및 단계별 환급 분류가 깨집니다. 이미 제거된 중복 sort를 다시 최적화한다고 주장하지 않습니다.
- **통과 조건:** 작업자 1/8개 모두 원시 표본·예산별 CDF·동일 시드 결과가 바이트 또는 비트 단위로 동일하고, 기존 [성능 회귀 테스트](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/tests/performance_regression.rs#L24)와 게임별 계약 검사가 통과해야 합니다. 개선이 없으면 **현재 정렬 구조 유지**.

### IMP-G02 · 예산별 누적 결과(prefix aggregate)로 반복 스캔 제거 (P2)

- **위치:** `src/domain/feedback/results.rs`의 `success_breakdown()`, `successful_means()`, 이들의 호출 지점. [근거 L112-L200](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/domain/feedback/results.rs#L112).
- **방법:** ① 한 시뮬레이션 결과를 사용자가 몇 번 조회하는지 계측합니다. ② 시나리오가 이미 `initial_flexible_required` 순으로 정렬되므로, 각 표본에서 필요한 누적 필드(`raw_pulls`, `returned`, 비용 등)의 `prefix[i+1] = prefix[i] + value[i]`를 **한 번** 생성합니다. ③ `partition_point`로 성공 표본의 오른쪽 경계를 얻고 해당 prefix 차이로 조건부 합계를 O(1)에 읽습니다. ④ 단순 확률 CDF 조회는 기존 `partition_point`를 그대로 사용합니다.
- **예외·위험:** `usize`/합계 overflow, 부동소수 합산 순서, 0개 성공, 동일 예산의 동률 처리에서 오차가 생길 수 있습니다. 부동소수 필드는 기존 반복 합산과 비교해 승인 가능한 공차를 먼저 명시하고, **정확히 같은 결과가 요구되면 순서와 계산 방식을 유지**합니다. 질의가 1회뿐이면 prefix 생성 비용이 이득보다 클 수 있으므로 도입하지 않습니다.

### IMP-G03 · 명조 세 전략의 제한된 병렬 스케줄 최적화 (P1)

- **위치:** `src/app/compute.rs`의 명조 3개 전략 분기·`thread::scope()` 및 8코어 CPU 상한. [근거 L80-L116](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/compute.rs#L80).
- **방법:** 전략별 `wall_ms`, `samples_per_s`, 실제 worker 수, CPU 사용률을 먼저 기록합니다. 전략 계산의 작업 편차가 크면 고정 코어 몫(`cpu_budget / 3`)과 **하나의 제한된 작업 큐**를 비교합니다. 각 전략/표본 ID의 난수 시드를 유지하고 전략 결과가 원래 순서로 삽입되도록 `strategy_id`를 유지합니다.
- **가드레일:** 현재는 3개 전략 결과가 UI 비교에 사용되므로 일부만 먼저 계산해 보여주는 방법은 사용자 기능 변경입니다. **선택 전략만 계산**은 기본 최적화에 포함하지 않습니다. 범위 내 속도 향상이 확인되지 않으면 기존 고정 분할 유지.

### IMP-G04 · 차트 계산/레이아웃 캐시와 이미지 텍스처 상한 (P2)

- **위치:** `src/ui/charts.rs`, `src/ui/dashboard/overview.rs`, `src/data/catalog/ui.rs`. [차트 근거](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/ui/charts.rs#L146), [텍스처 근거](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/data/catalog/ui.rs#L115).
- **방법:** ① 계산 완료 세대번호, 차트 너비, 그래프 표시 종류로 캐시 키를 구성해 동일 크기·동일 결과일 때 CDF 투영 좌표만 재사용합니다. ② `hover` 시 실제 확률은 **원본 CDF**에서 조회해 캐시 그래프의 표시 오차가 정답을 바꾸지 않게 합니다. ③ 이미지 캐시는 최근 사용 텍스처 수·GPU 메모리부터 측정하고, 상한이 필요할 때만 **화면에 보이지 않는** 항목을 만료시킵니다.
- **검증:** 창 크기 전환/게임 탭 변경/언어 변경/계산 결과 폐기/아이콘 재호출, 고DPI 화면에서 차트 정확성·프레임 p95·CPU idle 값을 비교합니다. CPU/GPU 메모리 절감이 없다면 캐시를 복잡하게 만들지 않습니다.

### 계산기 실험 승인 명령 및 원복 기준

```sh
cargo test --locked --all-targets
cargo test --locked --test v1_contracts
cargo test --locked --release --test performance_regression -- --ignored --nocapture
cargo clippy --locked --all-targets -- -D warnings
```

**계측 기록:** 표본 수 / 고정 시드·게임·환급 구성 / worker 수 / generation·sort·summary 단계 ms / Peak RSS / GUI frame p95 / 성공률·Wilson/DKW 구간 / 전후 CDF 최대 절대 오차. **즉시 원복:** 예산 차원의 뜻 변경, 표본 재현성 실패, 4개 게임의 천장/환급 차이, 중단한 계산의 오래된 결과가 UI에 표시되는 경우.


## 4.6 추가 보완 과제 — 관측성·정확성·회귀 안전성 (2026-11-11)

기존 IMP-G01~G04의 자료구조·차트 가속과 별개로, **실행 로그 비용과 계산 세대 안전성, 통계적 검증 증거**를 우선 보강할 필요가 있습니다. 아래 항목은 아직 코드에 반영하지 않은 개선안이며 실제 병목 또는 결함으로 확정한 내용이 아닙니다.

### IMP-G05 · 계산 중 동기 로그 flush 영향 측정 및 무손실 기록 정책 (P1 계측)

- **확인 근거:** [src/app/session_log.rs L118-L136](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/session_log.rs#L118-L136)에서 각 메시지마다 공유 Mutex를 잡고 파일에 한 줄 기록 후 flush합니다. 계산 요청과 결과는 [src/app/controller_session/calculation.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/controller_session/calculation.rs)에서 기록합니다.
- **개선 절차:** ① SSD/느린 디스크에서 1회 계산의 로그 행 수·잠금 대기·flush 누적 ms를 측정합니다. ② 지연이 유의미할 때만 기록 큐 + 전용 writer 또는 제한된 batch flush를 실험합니다. ③ 순서, 오류 기록, 현재 실행 log/session.log 경로, 새 실행 시 초기화, 종료 시 마지막 기록이 사라지지 않는 계약을 유지합니다. ④ 강제 종료와 정상 종료의 로그 내구성 차이를 명시하고 성능 때문에 무조건 flush를 삭제하지 않습니다.
- **검증/원복:** [session_log.rs L249-L318](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/session_log.rs#L249-L318)의 다중 스레드 로그 순서·재실행 초기화 테스트를 유지합니다. 수집된 행 수 또는 최종 SESSION_END가 누락되거나 디스크 장애 시 UI 표시가 악화되면 원복합니다.

### IMP-G06 · 입력 변경/연속 실행/취소가 겹치는 부하 회귀 시험 (P1)

- **확인 근거:** [calculation.rs L4-L25, L70-L72](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/controller_session/calculation.rs#L4-L25)는 세대 증가로 결과를 폐기하며, [compute.rs L40-L42, L158](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/src/app/compute.rs#L40-L42)는 취소 여부를 검사합니다.
- **개선 절차:** 시뮬레이션 실행 → 즉시 설정 변경 → 재실행 → 0.1초·0.5초 간격 반복 → 종료 순서로 테스트를 추가합니다. 중단된 스레드가 계속 장시간 CPU를 점유하지 않는지, 이전 세대 결과가 UI나 로그에서 최신 결과로 표시되지 않는지 검증합니다. 4개 게임 × 환급 ON/OFF × 작은/큰 표본 수를 분리합니다.
- **합격/원복:** 마지막 세대만 표시, 이전 요청의 결과·오류가 현재 상태를 덮어쓰지 않음, cancellation latency와 Peak RSS 악화 없음. 세대번호 검증을 성능 개선 대상으로 제거하는 것은 금지합니다.

### IMP-G07 · 성능 비교와 확률 검증의 증거를 분리해 보관 (P2)

- **확인 근거:** [tests/performance_regression.rs L38-L40, L85-L109](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/tests/performance_regression.rs#L38-L109)의 시간 측정은 의도적으로 ignored이며 절대 성능을 CI 합격 기준으로 단정하지 않습니다. [tests/v1_contracts.rs](https://github.com/P-JSL/gacha_pickup_calculator/blob/eedc576b2d67e04976ecf475341721a4dcce1a2b/tests/v1_contracts.rs)는 확률·목표·자원 계약을 검증합니다.
- **개선 절차:** 동일 커밋/CPU/OS/빌드 옵션/표본 수/시드를 가진 기준선 JSON에 실행 5회 이상의 중앙값·p95·최대 RSS와 정확성 검증 결과를 별도로 기록합니다. 윌슨/DKW 신뢰구간은 계산 결과의 **표본 불확실성**을 나타내며 성능 개선 효과나 실제 게임 규칙의 정확성을 보장하지 않는다고 명시합니다.
- **승인 기준:** 모든 게임 결과 계약이 우선 통과하고, 기준선과 변경본의 구성·해시·출력이 추적 가능해야 합니다. GitHub 공유 러너 한 번의 측정으로 개선율을 확정하지 않습니다.
