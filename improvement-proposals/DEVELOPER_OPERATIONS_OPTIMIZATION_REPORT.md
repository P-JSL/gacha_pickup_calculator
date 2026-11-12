# gacha_pickup_calculator — Codex 작업용 개선 계획서

> **기준 코드:** [`86a4733f54ce`](https://github.com/P-JSL/gacha_pickup_calculator/tree/86a4733f54ce0e20466cfa2c3e27c67253264834) · **문서 갱신:** 2026-11-12 (KST)  
> **상태:** 분석 및 작업 설계. 아래 작업은 별도 코드 커밋으로 수행해야 하며 이 문서 수정만으로 해결된 것이 아닙니다.  
> **대상:** 4종 게임 픽업·환급 확률 계산기(Rust/egui)

## 1. Codex가 먼저 지킬 작업 규칙

1. 해당 작업 ID 하나를 선택한 뒤 **현재 `main`의 실제 소스·호출자·테스트·설정**을 먼저 읽습니다. 이 문서의 기준 커밋은 분석 출처이지 수정할 때 고정해야 하는 오래된 버전이 아닙니다.
2. 작업 범위와 무관한 파일·포맷·공개 API·동작·스키마·리턴 데이터·CI 게이트를 바꾸지 않습니다. 존재하지 않는 메서드·옵션·경로를 만들지 않습니다.
3. 상태가 **기존 보호 유지** 또는 **실험·조건부**라면 결함으로 단정하거나 바로 리팩터링하지 않습니다. 먼저 재현 테스트 또는 프로파일을 만들고 필요성이 입증된 경우에만 변경합니다.
4. 변경 전 실패 사례를 재현하고, 최소 수정 후 기존 테스트 + 신규 회귀를 실행합니다. 실행하지 못한 Windows·모델·GUI·실제 장비 검증은 **미검증**으로 보고합니다.
5. 작업은 **하나씩** 구현·검증·커밋합니다. 완료 보고에는 변경 파일, 이전/이후 동작, 실행 명령·결과, 남은 위험, 커밋 SHA를 기록합니다. 사용자의 별도 요청 없이 릴리스·배포하거나 데이터를 삭제하지 않습니다.

## 2. 프로젝트 핵심 구조·보존 계약

시뮬레이션은 사용자가 실행 버튼을 누른 경우에만 시작하며, 게임별 천장·환급 규칙·보유 티켓을 이용한 계산 결과를 표시합니다. 이미지/카탈로그는 서명·SHA 기반 검증 경계를 갖고 실행별 로그를 기록합니다.

- 입력 변경 시 결과 무효화, 자동 재실행하지 않음. 오래된 계산 세대가 현재 결과를 덮어쓰지 않음.
- 붕괴: 스타레일·명조·스텔라 소라·이환의 게임별 천장, 4성 중복, 교환 재화, 캐릭터/장비 통화 규칙 보존.
- 표본 수·시드·환급 전후 결과가 동일한 비교 환경에서 확률 계산의 재현성과 무결성 확인.
- 원격 카탈로그는 검증된 서명·커밋·허용 경로·SHA 이후 게시하며 임의 실행 코드/미검증 이미지를 신뢰하지 않음.
- 시작 시 기존 세션 로그를 정리하되 심볼릭 링크를 따라 임의 파일을 덮어쓰지 않으며 기록 실패 시 경고.

## 3. 현재 확인된 사실과 미해결 조건

- 이미 구현: FFT 기반 일부 분포 계산, 이진 CDF 조회, 제한된 CPU 작업자, 1회 정렬, 계산 세대 구분, 이미지 SHA 검증.
- 미검증: 현행 버전의 실제 병목 비율과 전체 추가 가속 배수. 과거 국소 벤치마크 배수를 미래 개선량으로 사용 금지.
- 중단 기준: 성능 최적화가 시드/표본별 결과, 환급·천장 규칙, 새 실행 로그 정책 또는 공개 GUI 의미를 바꾸면 변경을 반려.

## 4. 작업 순서표

우선순위 **P0=정합성·데이터 보호**, **P1=검증·실측 필요**, **P2=조건부 최적화·사용성**입니다. 표의 '판정'은 이미 고쳐진 상태를 뜻하지 않습니다.

| ID | 우선 | 작업 | 현재 판정 |
|---|---|---|---|
| G-01 | P0 | 계산 결과·환급 계약 회귀 테스트 고정 | 기존 보호 유지 + 테스트 보강 |
| G-02 | P0 | 연속 실행·취소 시 오래된 결과 차단 검증 | 기존 세대 보호의 회귀 확장 |
| G-03 | P1 | 원격 카탈로그 갱신의 실패·복구 경계 | 새 실패 주입 검증 |
| G-04 | P1 | 이미지 무결성·캐릭터와 장비 연결 정합성 | 카탈로그-화면 교차 검사 |
| G-05 | P1 | 성능 기준선과 확률 정확도 증거 분리 | 계측 먼저 |
| G-06 | P2 | 정렬·예산 반복 집계의 추가 개선 여부 판단 | 측정 후 조건부 실험 |
| G-07 | P2 | 명조 3전략 병렬 작업 분배 검토 | 실측 후 실험 |
| G-08 | P2 | 차트·이미지 렌더 지연 독립 최적화 | 프레임 프로파일링 후 |
| G-09 | P2 | 세션 로그 flush 비용과 유실 방지 | 운영 계약 선행 |
| G-10 | P2 | 키보드 접근성·릴리스 자산 검증 | 별도 UI·릴리스 검증 |

## 5. Codex 실행용 상세 작업지시서

### G-01 · 계산 결과·환급 계약 회귀 테스트 고정 (P0)

- **판정:** 기존 보호 유지 + 테스트 보강
- **수정·확인 파일:** [`src/domain/feedback/rolls.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/rolls.rs), [`src/domain/feedback/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/tests.rs), [`tests/v1_contracts.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/tests/v1_contracts.rs)
- **구현 지시:** 4개 게임별 최소/최대 목표, 첫 픽업 확정·미확정, 4성 전부 풀돌, 환급 재사용, 예산 0/상한, NaN·비정상 pity 입력을 구분하는 고정 시드 회귀 fixture와 불변식 테스트를 추가합니다. 새 게임 규칙을 추정하거나 고정된 비교 결과를 편의상 바꾸지 마세요.
- **근거 및 관찰 사실:** `src/domain/feedback/rolls.rs`의 게임 규칙과 `tests/v1_contracts.rs`의 공개 결과 계약을 비교하는 회귀 작업입니다. 결함이 재현된 것으로 확정하지 않습니다.
- **적용 원리·수식 또는 불변식:** 성공확률 추정값 `p̂ = 성공 표본수 / 총 표본수`, 재화 보존 `최종 보유 = 초기 보유 − 소비 + 실제 환급 + 허용 교환`을 별개로 검증합니다. 재투입 환급은 이벤트 순서대로 계산하여 중복 계상하지 않습니다.
- **공식 문서·논문 출처:** [NIST 통계공학 핸드북](https://www.itl.nist.gov/div898/handbook/), [Cargo 테스트 공식 문서](https://doc.rust-lang.org/cargo/commands/cargo-test.html) — 표본 추정과 게임 규칙의 정확성은 다른 검증 문제입니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 동일 입력·시드의 결과와 재화 흐름이 바뀌지 않고 신규 회귀가 통과
- **중단·원복:** 확률·교환 결과가 바뀌면 관련 최적화를 중단
- **기존 보고서 연결:** Q-G01, Q-G03

### G-02 · 연속 실행·취소 시 오래된 결과 차단 검증 (P0)

- **판정:** 기존 세대 보호의 회귀 확장
- **수정·확인 파일:** [`src/app/controller_session/calculation.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/controller_session/calculation.rs), [`src/app/compute.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/compute.rs), [`src/app/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/tests.rs)
- **구현 지시:** 실행→입력 수정→즉시 재실행→취소→다시 실행을 반복하는 테스트를 작성합니다. 세대번호 증가와 취소 플래그를 보존하고 이전 worker가 최신 UI/로그 성공 결과를 덮어쓰지 않게 확인합니다.
- **근거 및 관찰 사실:** `src/app/controller_session/calculation.rs`에서 `generation`을 증가시키고 `active_generation`을 갱신합니다(기준 소스 7~21행). 기존 구현을 삭제하지 않고 경합 케이스를 검증합니다.
- **적용 원리·수식 또는 불변식:** 불변식 `게시(result) ⇒ result.generation == active_generation`. 동시에 두 작업 A,B가 완료되더라도 최신 세대만 결과에 반영되어야 합니다.
- **공식 문서·논문 출처:** [Rust 원자성·메모리 순서](https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html) — Relaxed/Acquire-Release 교체는 해당 결과 전달 경로를 확인한 다음 결정합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 마지막 세대만 결과를 게시하고 중단된 작업의 오류가 현재 세대에 표시되지 않음
- **중단·원복:** 세대 가드·취소 체크 제거 금지
- **기존 보고서 연결:** IMP-G06, Q-G02

### G-03 · 원격 카탈로그 갱신의 실패·복구 경계 (P1)

- **판정:** 새 실패 주입 검증
- **수정·확인 파일:** [`src/data/runtime_updates.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/runtime_updates.rs), [`src/data/runtime_updates/cache.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/runtime_updates/cache.rs), [`src/data/runtime_updates/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/runtime_updates/tests.rs)
- **구현 지시:** 서명 실패, 허용되지 않은 URL, 중단된 다운로드, 손상 캐시, 검증 직전 프로세스 종료를 조합합니다. 이미 검증된 카탈로그를 유지하거나 명시적으로 실패하도록 테스트를 작성하고 서명 검증을 우회하는 캐시 최적화를 금지합니다.
- **근거 및 관찰 사실:** `src/data/runtime_updates.rs`와 하위 캐시가 외부 카탈로그를 다루는 경계입니다. 현재 경로·검증 방법은 파일 자체를 열어 재확인하고 결함을 추정하지 마세요.
- **적용 원리·수식 또는 불변식:** 갱신의 상태 전이 `다운로드 → 신뢰 검증 → 스테이징 → 활성화`; 불변식 `활성화 ⇒ 전체 자산 검증 성공`. 실패 시 마지막 정상 버전 또는 명시적 오류 상태입니다.
- **공식 문서·논문 출처:** [NIST SSDF SP 800-218](https://csrc.nist.gov/pubs/sp/800/218/final), [SLSA Provenance](https://slsa.dev/spec/v1.2/provenance) — 서명·공급망 출처와 파일 무결성은 구별합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 검증되지 않은 번들이 활성화되지 않고 기존 정상 데이터가 오염되지 않음
- **중단·원복:** 커밋 SHA 존재만으로 신뢰 판정 금지
- **기존 보고서 연결:** Q-G04, Q-G07

### G-04 · 이미지 무결성·캐릭터와 장비 연결 정합성 (P1)

- **판정:** 카탈로그-화면 교차 검사
- **수정·확인 파일:** [`src/data/security.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/security.rs), [`src/data/catalog.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/catalog.rs), [`src/data/catalog/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/catalog/tests.rs)
- **구현 지시:** 내장 SHA 매니페스트 검증을 유지하면서 캐릭터 선택 시 전용 장비 자동 연동이 틀리거나 누락된 관계를 데이터 기반으로 검사합니다. 이미지 경로 변조/동일 크기 위조/미등록 관계를 테스트합니다.
- **근거 및 관찰 사실:** `src/data/security.rs`의 자산 신뢰 경계와 `src/data/catalog.rs`의 이름/장비 관계를 따로 확인합니다. 카탈로그 값의 게임 정당성은 저장소 데이터와 공식 게임 데이터가 다를 수 있습니다.
- **적용 원리·수식 또는 불변식:** 정합성 조건 `선택한 캐릭터의 장비_ID == 연결 관계 테이블의 장비_ID` 및 `SHA256(실제 파일) == 기대 해시`. 두 검사는 서로 대체되지 않습니다.
- **공식 문서·논문 출처:** [NIST FIPS 180-4 SHA](https://csrc.nist.gov/pubs/fips/180-4/upd1/final) — 체크섬은 무결성 도구이며 퍼블리셔의 진위까지 자동 보장하지 않습니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 잘못된 캐릭터-장비 관계 및 변조 이미지가 성공으로 표시되지 않음
- **중단·원복:** 확인되지 않은 최신 게임 데이터를 임의 삽입하지 않음
- **기존 보고서 연결:** Q-G05, Q-G08

### G-05 · 성능 기준선과 확률 정확도 증거 분리 (P1)

- **판정:** 계측 먼저
- **수정·확인 파일:** [`src/domain/feedback/runner.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/runner.rs), [`tests/performance_regression.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/tests/performance_regression.rs), [`docs/PERFORMANCE.md`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/docs/PERFORMANCE.md)
- **구현 지시:** 100k/1M 표본, 게임별 캐릭터·장비, 1/2/4/8 CPU 작업자를 조건으로 추첨/정렬/집계/GUI 시간을 분리 계측합니다. 동일 하드웨어·시드·빌드·표본 수의 중앙값·p95·최대 RSS와 결과 검증을 별도로 기록합니다.
- **근거 및 관찰 사실:** `runner.rs`의 `simulate_chunk()`(기준 85행)와 `sort_unstable_by_key()`(기준 210행)를 개별 계측하는 근거가 있습니다. 현재 1회 정렬 최적화는 이미 구현됨을 확인했습니다.
- **적용 원리·수식 또는 불변식:** `T_total = T_setup + T_sample + T_sort + T_aggregate + T_render`로 단계 분해. 개선 대상 비율 `f`와 국소 속도 배수 `r` 측정 후 `S = 1/(1-f+f/r)`로 전체 상한 계산; 실제 수치가 없으면 전망 불가.
- **공식 문서·논문 출처:** [Amdahl 원 논문](https://doi.org/10.1145/1465482.1465560), [Google Benchmark 반복 측정](https://google.github.io/benchmark/user_guide.html) — 국소 FFT의 기존 가속을 전체 향상으로 환산하지 마세요.
- **검증 명령:** `cargo test --locked --release --test performance_regression -- --ignored --nocapture`
- **완료 조건:** 기준선과 변경본 결과 SHA·실측 환경이 남아 있으며 과거 국소 가속 배수를 전체 가속이라고 표기하지 않음
- **중단·원복:** 프로파일 없이 가속 배수 확정 금지
- **기존 보고서 연결:** IMP-G01, IMP-G07

### G-06 · 정렬·예산 반복 집계의 추가 개선 여부 판단 (P2)

- **판정:** 측정 후 조건부 실험
- **수정·확인 파일:** [`src/domain/feedback/runner.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/runner.rs), [`src/domain/feedback/results.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/results.rs)
- **구현 지시:** 현재 이미 구현된 1회 정렬을 다시 제거 과제로 만들지 않습니다. 정렬이 충분히 큰 병목일 때만 제한된 티켓 범위 K의 히스토그램과 기존 비교정렬을 비교합니다. 예산 반복 조회가 많을 때만 누적합(prefix) 준비 비용을 포함한 개선을 시험합니다.
- **근거 및 관찰 사실:** `src/domain/feedback/runner.rs`에서 사전 예약 후 **이미 한 번만** 정렬합니다. 새로운 정렬 방식의 후보는 현재 `tickets` 범위와 quantile에 필요한 원시 시나리오 관계를 확인한 후 판단합니다.
- **적용 원리·수식 또는 불변식:** 기존 비교 정렬은 `O(N log N)`. `K`개의 유한한 정수 티켓 비용만 다루는 계수표는 `O(N+K)`·추가공간 `O(K)`이지만 원본 시나리오/동점 의미가 사라지면 동등 구현이 아닙니다.
- **공식 문서·논문 출처:** [C++ sort 복잡도](https://en.cppreference.com/w/cpp/algorithm/sort), [Rust slice 정렬](https://doc.rust-lang.org/std/primitive.slice.html) — 복잡도 표기는 후보 상한이고 실측 가속 보장은 아닙니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 정렬 순서·분위수에 연결된 원본 시나리오·환급 결과 동일, 메모리 증가 상한 확인
- **중단·원복:** K가 크거나 단일 조회여서 더 느리면 적용하지 않음
- **기존 보고서 연결:** IMP-G01, IMP-G02

### G-07 · 명조 3전략 병렬 작업 분배 검토 (P2)

- **판정:** 실측 후 실험
- **수정·확인 파일:** [`src/app/compute.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/compute.rs), [`src/domain/feedback/runner.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/runner.rs)
- **구현 지시:** 기존 1~8 작업자 제한과 세 전략별 CPU 예산을 유지하면서 각 전략 시간과 idle 구간을 측정합니다. 불균형이 입증될 때만 제한된 동적 chunk 크기를 비교합니다.
- **근거 및 관찰 사실:** `src/app/compute.rs`에 3전략별 CPU 예산 조절이 이미 존재합니다. 예산 정책/seed 분할/중첩 병렬을 먼저 파악하고 변경하세요.
- **적용 원리·수식 또는 불변식:** 이론적 병렬 상한 `T_P ≥ max(T_1/P, T_∞)`; 병렬 작업자 수 `P`가 증가해도 가장 긴 종속 실행 경로 `T_∞`는 남습니다. 합계 작업 시간과 총 wall-time을 함께 기록합니다.
- **공식 문서·논문 출처:** [Work Stealing 분석](https://doi.org/10.1145/324133.324234) — 해당 논문의 스케줄 정책이 이 프로젝트에 설치되어 있다고 단정하지 않습니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 과잉 스레드 없이 입력별 출력·표본 재현성 유지, 실제 총 수행 시간 단축 확인
- **중단·원복:** 시드·난수 호출 순서 또는 취소 응답을 바꾸면 반려
- **기존 보고서 연결:** IMP-G03

### G-08 · 차트·이미지 렌더 지연 독립 최적화 (P2)

- **판정:** 프레임 프로파일링 후
- **수정·확인 파일:** [`src/ui/charts.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/ui/charts.rs), [`src/data/catalog/ui.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/catalog/ui.rs)
- **구현 지시:** 차트 지오메트리 구성/텍스처 업로드의 frame p95를 측정합니다. 실제 UI 병목일 때만 결과 세대·화면 너비 기준 유효기간이 정해진 캐시와 보이는 이미지 중심 텍스처 상한을 구현합니다.
- **근거 및 관찰 사실:** `src/ui/charts.rs`와 `src/data/catalog/ui.rs`가 지오메트리·텍스처 처리 후보입니다. GUI 성능은 계산 단계에서 분리 계측해야 합니다.
- **적용 원리·수식 또는 불변식:** 렌더 지연 `T_frame = T_layout + T_mesh + T_upload + T_paint`(계측용 구분)와 초당 프레임 `FPS ≈ 1/T_frame(초)`. 화면폭·데이터 세대가 바뀌면 캐시를 무효화합니다.
- **공식 문서·논문 출처:** [egui 공식 문서](https://docs.rs/egui/latest/egui/), [W3C 사용자 입력 응답 관련 참고](https://www.w3.org/TR/WCAG22/) — 웹 기준은 데스크톱 egui의 인증 기준이 아닌 사용성 참고입니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 그래프 값·레이블·이미지 관계 불변, frame p95 개선 및 캐시 상한 확인
- **중단·원복:** 메모리 누수나 다른 세대 그래프 재사용이면 원복
- **기존 보고서 연결:** IMP-G04

### G-09 · 세션 로그 flush 비용과 유실 방지 (P2)

- **판정:** 운영 계약 선행
- **수정·확인 파일:** [`src/app/session_log.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/session_log.rs)
- **구현 지시:** 메시지별 flush 누적시간을 계측한 후 필요한 경우에만 기록 전용 writer/제한된 배치 쓰기를 실험합니다. 실행 재시작 시 초기화, 심볼릭 링크 거부, 파일 경로 경고, 로그 창 20,000줄 제한을 유지합니다.
- **근거 및 관찰 사실:** `session_log.rs` 71~82행은 심볼릭 링크 거부 후 truncation, 126행은 각 기록 뒤 `flush` 호출입니다. 비용 후보가 존재한다는 뜻이지 실제 병목이라는 뜻은 아닙니다.
- **적용 원리·수식 또는 불변식:** `T_log = Σ(T_lock_i + T_write_i + T_flush_i)`를 계측. 배치 주기 Δ를 늘리면 마지막 Δ 사이의 내구성 위험이 발생할 수 있으므로 허용 유실량 0이라는 계약을 먼저 확인합니다.
- **공식 문서·논문 출처:** [Rust `std::io::Write::flush`](https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.flush), [NIST SSDF](https://csrc.nist.gov/pubs/sp/800/218/final) — `flush`와 디스크 영속성 `fsync`는 동일하지 않습니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 실제 누적 로그 지연이 감소하고 정상 종료의 기록 누락·순서 변경 없음
- **중단·원복:** 기록 손실을 수용하지 않는 상태에서 flush를 무조건 제거하지 않음
- **기존 보고서 연결:** IMP-G05, Q-G06

### G-10 · 키보드 접근성·릴리스 자산 검증 (P2)

- **판정:** 별도 UI·릴리스 검증
- **수정·확인 파일:** [`src/ui/dashboard.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/ui/dashboard.rs), [`.github/workflows/rust.yml`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/.github/workflows/rust.yml), [`scripts/verify_release_contract.py`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/scripts/verify_release_contract.py)
- **구현 지시:** 키보드 전용 실행/설정/결과 열람·포커스 가시성을 점검하고, Windows 배포 ZIP/EXE 버전·카탈로그·이미지 매니페스트의 출처 및 SHA 검증을 CI 계약에 추가할 필요가 있는지 기존 workflow를 확인합니다.
- **근거 및 관찰 사실:** UI 접근성은 코드와 실제 사용자 입력으로 확인해야 하는 제안입니다. `scripts/verify_release_contract.py`와 Rust CI의 버전·이미지 번들 규칙은 이미 존재합니다.
- **적용 원리·수식 또는 불변식:** 릴리스 기준 `SHA256(배포 EXE) == 매니페스트 기재 해시`, 스크린리더·키보드 흐름은 주요 기능별 `접근 가능한 조작 경로 ≥ 1`로 테스트합니다.
- **공식 문서·논문 출처:** [WCAG 2.2 키보드 원칙](https://www.w3.org/WAI/WCAG22/Understanding/keyboard), [SLSA Provenance](https://slsa.dev/spec/v1.2/provenance) — 데스크톱 앱에는 플랫폼별 접근성 테스트를 적용합니다.
- **검증 명령:** `python3 scripts/verify_release_contract.py`
- **완료 조건:** 접근 가능한 주요 조작 경로와 검증 가능한 빌드 자산 정보 확보
- **중단·원복:** 이미 있는 CI 검사를 중복 추가하거나 출처/서명 검증을 약화하지 않음
- **기존 보고서 연결:** Q-G09, Q-G10

## 6. 개발·CI 검증 순서

```bash
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --release --test performance_regression -- --ignored --nocapture
python3 scripts/verify_release_contract.py
```

위 명령은 저장소에서 확인한 빌드·워크플로와 테스트를 기준으로 작성했습니다. Linux GUI 의존성, Windows 실행 환경, 실모델·실데이터 등 필요한 조건이 없으면 실패 원인을 분리하고 **실행 성공으로 보고하지 마세요**. 시간·메모리·처리량은 동일 머신과 입력에서 기준선/변경본을 각각 반복 측정한 경우에만 비교합니다.

## 7. 이력 통합·분류 원칙

- 기존 문서의 `IMP-*` 항목, 별도의 `Q-*` 품질 검토 및 성능 대안표는 위 작업 ID에 연결해 **중복된 과제를 하나로 합쳤습니다**. 위에 별도 표기하지 않은 과거 제안은 현행 구현에 이미 포함됐거나 근거·계측이 부족한 조건부 아이디어로 간주합니다.
- 이전의 **100회/1,000회 반복 검토**는 합성 시나리오의 모델 검산으로, 실제 소프트웨어를 같은 횟수만큼 실행·프로파일링했다는 뜻이 아닙니다. 산출물은 별도 검산 패키지로 보존하고 여기에는 실행 작업에 필요한 결론만 반영합니다.
- `P0`는 위험의 우선순위이지 이미 장애가 발생했다는 판정이 아닙니다. 보안 이론·논문의 다른 시스템 개선 배수를 이 프로젝트의 성과로 옮기지 않습니다.
- **최종 완료 판정:** 확인된 불일치는 실제 소스 수정 및 테스트가 통과해야만 '해결'로 변경합니다. 이 문서는 **Codex 작업 지침**이며 구현 완료 증거가 아닙니다.

## 8. 추가 보안·위험·복구 보완 사항 — 소스 재검증 (2026-11-12)

이번 보완은 기존 **10개 작업을 대체하거나 수행 완료로 처리하지 않습니다**. 실제 소스에서 확인한 위험 경로와 테스트가 필요한 가정을 구분했으며, 공개된 취약점이나 악용 성공을 확인했다는 뜻이 아닙니다. 보안·무결성 작업은 실제 장애 재현→최소 수정→회귀 검증 순으로 진행하십시오.

### 8.1 재검증에서 추가로 확인한 위험과 경계

1. 캐시 게시 함수는 Windows에서 기존 목적 파일을 제거한 다음 새 임시 파일을 rename합니다. 이 **두 호출 사이의 장애 복구 가능성**을 검증해야 합니다.
2. CSV 보안 로더는 파일을 최대 8MiB로 제한하지만 관측 데이터 파서는 행을 `Vec`로 수집합니다. 바이트 제한과 행·힙 제한은 다른 문제입니다.
3. GitHub 커밋 신뢰 검증은 응답 문자열에서 특정 JSON 조각을 검색합니다. 현재 취약점으로 단정하지 않고 의미가 같은 JSON 표현과 잘못된 응답의 경계를 검사합니다.
4. 캐시 위치 결정은 일부 환경변수가 없을 때 시스템 임시 경로로 폴백합니다. 계정 간 격리와 재시작 지속성에 대한 운영 규칙이 필요합니다.

### 8.2 추가 Codex 작업 목록

| 작업 ID | 우선순위 | 신규 보완 과제 | 증거 수준 |
|---|---|---|---|
| **G-11** | P1 | Windows 캐시 파일 교체의 중간 장애·복구 검증 | 확인된 비원자적 교체 순서·장애 발생은 미검증 |
| **G-12** | P1 | 8MiB CSV의 극단적 다행 입력 메모리·시간 상한 | 확인된 용량 제한 + 미검증 행수 비용 |
| **G-13** | P2 | GitHub 커밋 검증의 JSON 구조·필드 신뢰 재검토 | 문자열 검사 구현 확인, 실제 우회는 미확인 |
| **G-14** | P2 | 환경변수 누락 시 캐시 임시 디렉터리 격리 | 조건부 경로·환경 위험, 실제 권한 취약점 미확정 |

### 8.3 작업별 상세 근거·검증·원복 기준

### G-11 · Windows 캐시 파일 교체의 중간 장애·복구 검증 (P1)

- **확인 수준:** 확인된 비원자적 교체 순서·장애 발생은 미검증
- **근거 소스:** [`src/data/runtime_updates/cache.rs` 99-139행](https://github.com/P-JSL/gacha_pickup_calculator/blob/a8a024d97c80a983437babf6cb58591d2b1c9963/src/data/runtime_updates/cache.rs#L99-L139)
- **추가 판단 근거:** `write_atomic()`의 Windows 경로에서 목적 파일 `remove_file()` 이후 `rename()`이 수행됩니다. 기존 G-03(갱신 검증)은 유지하고 **게시 직전 실패**만 별도 대상으로 삼습니다.
- **Codex 구현·시험 지시:** 임시 작성 완료/기존 삭제 직후/새 이름 교체 실패/디스크 부족/권한 거부/동시 시작을 순서별로 주입합니다. 현재 검증된 번들이 있으면 마지막 정상 데이터가 유지되거나 복구 경로가 명시되어야 합니다. Windows 파일 교체 API나 백업 이름을 쓰는 대안은 기존 의존성·파일 권한을 확인한 뒤 선정합니다.
- **수식·안전 불변식:** `기존 유효 캐시 ∧ 교체 실패 ⇒ 기존 캐시 사용 가능 ∨ 신뢰된 백업으로 복구 가능`. 새 활성 리비전 표시는 검증된 대상만 가리켜야 합니다.
- **검증 실행:** `cargo test --locked --all-targets`; Windows 전용 임시 디렉터리 장애주입 테스트를 신규 추가해 실행
- **완료·합격 기준:** 윈도우에서 여러 실패 지점마다 검증되지 않은 캐시가 활성화되지 않고 마지막 정상 데이터가 손실 없이 복구됨
- **위험·원복 조건:** 캐시 포맷이나 배포 채널 변경 금지, 임시 갱신 실패를 단순 성공으로 표시 금지
- **기존 작업 연결:** G-03 / Q-G07
- **공식 명세·기술 출처:** [원문 참고](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)

### G-12 · 8MiB CSV의 극단적 다행 입력 메모리·시간 상한 (P1)

- **확인 수준:** 확인된 용량 제한 + 미검증 행수 비용
- **근거 소스:** [`src/analysis/observations/parser.rs` 6-40행](https://github.com/P-JSL/gacha_pickup_calculator/blob/a8a024d97c80a983437babf6cb58591d2b1c9963/src/analysis/observations/parser.rs#L6-L40)
- **추가 판단 근거:** CSV 로더의 최대 입력 8MiB 제한과 별개로 파서가 모든 유효 행 번호·텍스트 쌍을 `Vec<(usize,&str)>`로 생성합니다. 짧은 행 수백만 개가 가능한지 실측 확인이 필요합니다.
- **Codex 구현·시험 지시:** 빈 행/짧은 행 반복, 컬럼 64개 경계, 큰 행/마지막 행 누락·BOM을 조합한 fixture를 만들고 peak RSS와 처리시간을 계측합니다. 행 제한이나 스트리밍 파싱은 기존 정상 CSV와 관측 계산 결과 유지가 증명될 때만 채택합니다.
- **수식·안전 불변식:** `메모리 상한 != 파일 바이트 상한`; 파서 임시 메모리는 대략 `O(행수 + 필드수 + 출력수)`이며 모든 행을 수집하면 파일 크기만으로 RSS를 예측할 수 없습니다.
- **검증 실행:** `cargo test --locked --all-targets`; 테스트에 임의 큰 입력 대신 합성 fixture·RSS 측정 추가
- **완료·합격 기준:** 정상 8MiB CSV 결과 불변, 악성 또는 비정상 다행 입력이 프로그램 전체 메모리를 무제한 소모하지 않음
- **위험·원복 조건:** 기존 정상 고객 CSV를 정책 합의 없이 거부하지 않도록 호환성 확인
- **기존 작업 연결:** G-01 / G-05
- **공식 명세·기술 출처:** [원문 참고](https://owasp.org/www-community/attacks/Denial_of_Service)

### G-13 · GitHub 커밋 검증의 JSON 구조·필드 신뢰 재검토 (P2)

- **확인 수준:** 문자열 검사 구현 확인, 실제 우회는 미확인
- **근거 소스:** [`src/data/runtime_updates/network.rs` 90-110행](https://github.com/P-JSL/gacha_pickup_calculator/blob/a8a024d97c80a983437babf6cb58591d2b1c9963/src/data/runtime_updates/network.rs#L90-L110)
- **추가 판단 근거:** `verified_github_commit()`가 `response.starts_with()`와 여러 `contains()`를 이용해 SHA·verification·author/committer를 판정합니다. 현재 입력은 GitHub REST 응답이나, JSON 필드 순서·정규화·중첩 형식을 구조적으로 보장한 파서는 아닙니다.
- **Codex 구현·시험 지시:** 동일 의미의 정상 JSON(필드 순서·공백 차이), 거짓 verification·다른 author, 임의 메타데이터 필드에 문자열 삽입, 인증 실패/필드 누락을 테스트합니다. 구조적 JSON 파싱이 필요하면 현재 Cargo 의존성 정책을 먼저 확인해 최소 변경으로 적용합니다.
- **수식·안전 불변식:** `신뢰된 커밋 ⇒ response.sha == pinned_sha ∧ response.verification.verified == true ∧ reason == valid ∧ 기대 식별자 일치`; 필드값은 원래 JSON 노드에서 직접 검사되어야 합니다.
- **검증 실행:** `cargo test --locked --all-targets` 및 잘못된/정상 응답 fixture 회귀
- **완료·합격 기준:** 잘못된 응답은 모두 거부하고 순서만 다른 정상 JSON을 불필요하게 거부하지 않음, 네트워크 allowlist 유지
- **위험·원복 조건:** GitHub의 verified 플래그가 저장소 관리자 계정 탈취까지 막는다고 주장 금지
- **기존 작업 연결:** G-03 / G-10
- **공식 명세·기술 출처:** [원문 참고](https://docs.github.com/en/rest/commits/commits#get-a-commit)

### G-14 · 환경변수 누락 시 캐시 임시 디렉터리 격리 (P2)

- **확인 수준:** 조건부 경로·환경 위험, 실제 권한 취약점 미확정
- **근거 소스:** [`src/data/runtime_updates/cache.rs` 3-19행](https://github.com/P-JSL/gacha_pickup_calculator/blob/a8a024d97c80a983437babf6cb58591d2b1c9963/src/data/runtime_updates/cache.rs#L3-L19)
- **추가 판단 근거:** `LOCALAPPDATA`·유효한 `XDG_CACHE_HOME`·`HOME`이 없으면 `std::env::temp_dir()/gacha_pickup_calculator_updates`가 사용됩니다. 공유 시스템의 임시 영역과 재시작 유지 정책은 별도 확인이 필요합니다.
- **Codex 구현·시험 지시:** 환경변수를 조합해 캐시 루트를 표시하고, 다른 사용자 계정/비정상 권한/미리 설치된 링크/프로세스 동시 실행 조건을 임시 테스트 계정으로 검증합니다. 안정된 개인별 저장 위치가 없을 때는 검증된 캐시 사용 정책을 명시적으로 결정합니다.
- **수식·안전 불변식:** `cache_root(A) != cache_root(B)`가 필요한 다중 사용자 환경에서는 계정별 격리 보장; `경로가 신뢰되지 않음 ⇒ 임의 파일 덮어쓰기 불가`. 다만 운영체제 기본 임시폴더의 실제 격리 여부를 확인합니다.
- **검증 실행:** `cargo test --locked --all-targets` + 계정별 임시 캐시 경로 테스트
- **완료·합격 기준:** 신뢰되지 않은 폴더/링크를 통해 다른 사용자 파일을 바꾸지 않으며 읽기 실패는 명시적으로 보고
- **위험·원복 조건:** 개인 환경에서 동작하던 캐시를 예고 없이 삭제·이동하지 않음
- **기존 작업 연결:** G-03 / G-09
- **공식 명세·기술 출처:** [원문 참고](https://doc.rust-lang.org/std/env/fn.temp_dir.html)

### 8.4 보안 및 완료 판정 규칙

- **P0/P1/P2는 업무 우선순위입니다. CVSS 점수나 실제 침해사고 판정이 아닙니다.** 각 원인의 악용 가능성·권한·사용 환경은 테스트로 확인합니다.
- 실데이터·고객 문서 대신 합성 입력과 제한 권한의 임시 경로를 사용합니다. 전원 종료, 파일 교체, 계정 충돌 등 파괴적 장애 주입은 **운영 파일에 수행하지 않습니다**.
- **기존 보호·서비스 계약을 유지**하고 수정 대상 파일·함수 및 기존 호출자를 재확인합니다. 이미 구현된 안전 장치를 제거하거나 테스트만 통과하도록 약화하지 않습니다.
- 변경 전 실패 재현, 변경 후 정상·비정상 경계 테스트, OS별 차이, 부분 실패·롤백, 배포 영향, 테스트 실행 로그를 해당 작업 ID에 기록합니다. 실측/실행하지 않은 검사는 완료로 기록하지 않습니다.
- 위 출처는 방법론·플랫폼 동작의 근거이며 **해당 프로젝트에서 공격이 성공했음을 증명하지 않습니다**. 다른 시스템의 보안 사고·수치·논문 결과를 그대로 전이하지 않습니다.

## 9. 2026-11-12 구현 이력 — PR #60

이 절은 **코드 변경·테스트 추가 이력**입니다. 테스트 코드가 추가됐다는 사실과 특정 플랫폼에서 검증에 성공했다는 사실을 혼동하지 않습니다. 최종 CI 실행 결과는 [PR #60](https://github.com/P-JSL/gacha_pickup_calculator/pull/60)의 최신 HEAD에 연결된 체크에서 확인합니다.

| ID | 이번 변경 | 검증 및 보존 범위 |
|---|---|---|
| G-01 | 4개 게임의 최대 캐릭터·장비 목표, 첫 픽업 확정/미확정, 4성 최대 돌파 가정의 고정 시드·1/4 worker 동등성 테스트 추가 | 게임 규칙·RNG 시드·결과 배열·평균 환급 불변 |
| G-02 | 반복 실행→구세대 취소→재실행→최신 결과 비교 테스트 추가 | 기존 `generation`·`active_generation` 가드 유지. 실제 Windows GUI 동시 클릭은 별도 수동 검증 |
| G-03 | 분리된 캐시 루트에서 정상 활성 버전·손상 후보·잘못된 마커·손상된 활성 캐시 테스트 추가 | 네트워크 실서버·디스크 고장 주입은 이번 테스트 범위 밖 |
| G-04 | PNG의 길이·매직 헤더를 그대로 둔 단일 바이트 변조 테스트 추가 | 기존 679개 SHA 매니페스트와 141개 전용 장비 매핑 테스트 유지 |
| G-05 | 기존 기본 CI 성능 회귀 유지 + 4개 게임·10만/100만 표본·1/2/4/8 작업자 고정 시드 행렬 벤치마크 및 표본/정렬 단계별 계측 추가 | **계측 도구 구현**. 고비용 전체 행렬은 `GACHA_PERF_MATRIX=1`을 명시한 경우에만 실행. 동일 머신 실측 기준선/비교값 미확보 |
| G-06 | 기존 1회 `sort_unstable_by_key((budget,raw))` 유지, 계수표+prefix CDF 비교/결과 일치성 검사 추가 | **대안 측정 코드 구현**, 경로별 연결 데이터·동점 순서를 재구성하지 못하는 히스토그램으로 대체하지 않음 |
| G-07 | 명조 0·1·2 교환 전략별 worker 수/실행시간/오류를 `WUWA_STRATEGY_TIMING` 이벤트에 기록 | **실행 시 계측 구현**, 기존 8코어 제한/시드/동기·병렬 분기 유지. 동적 분배는 불균형 실측 전 보류 |
| G-08 | `GACHA_PROFILE_UI=1`을 지정했을 때 차트 지오메트리(64개 표본)·이미지 텍스처 제출(8개 표본)의 median/p95/max 계측 | **CPU 제출 경로 계측 구현**, GPU present 및 실제 frame p95는 별도 시스템 프로파일 필요. 캐시 정책은 유지 |
| G-09 | 로그 writer의 Mutex 대기/쓰기/flush 누적 시간·횟수·오류를 기록, 앱 정상 종료 시 `LOG_IO_SUMMARY` 기록 | **비용 계측 구현**, 건별 `flush()`·재시작 파일 초기화·`log/session.log` 경로 유지. 배치 flush는 유실 정책 확정 전 보류 |
| G-10 | `docs/CI_CHANGE_SCOPES.md`의 배포 파일 수(4종)와 `log/` 계약을 실제 워크플로 기준으로 정정 | 기존 SHA/공급망 증명 게이트 유지. 키보드·스크린리더 접근성은 **미검증** |
| G-11 | Windows의 기존 파일 **삭제 후 rename** 경로를 백업을 사용한 `ReplaceFileW`로 교체. 실패/복구 테스트 추가 | 기존 캐시의 비원자적 빈 구간 축소. 전원 차단·파일시스템 장애의 모든 결과를 입증한 것은 아님 |
| G-12 | CSV 전체 비주석 행 번호 `Vec` 적재 제거; 스트리밍 중 최대 200,000행 제한. BOM·64열·원본 줄번호 테스트 | 기존 8 MiB 제한과 출력 스키마 유지. 실제 peak RSS 비교는 추가 계측 필요 |
| G-13 | 커밋 신뢰 검사를 JSON 객체의 `sha`, `commit.verification`, `commit.committer`, `author.login` 경로별로 검증. 중복 키·위장 중첩·잘못된 구조 거부 | 신규 dependency 없이 구조/중복 키 검사. GitHub 계정 침해까지 방어한다고 주장하지 않음 |
| G-14 | 환경변수 누락 시 임시 폴더 폴백의 격리 정책 재검토 | 운영 계정·경로 권한·캐시 지속성에 대한 확정 조건이 없어 **미착수**, 기존 동작 유지 |

### 변경 전후 중요 계약

- **변경하지 않은 것:** 4개 게임의 천장·픽업·환급 공식, 시뮬레이션 시드, 화면의 수동 실행, 실행 로그 `log/session.log` 생성·초기화, PNG 식별자와 외부 연결 규격.
- **변경된 것:** 신뢰 검증·CSV 임시 메모리·Windows 캐시 게시 및 이를 검사하는 테스트. 정상 GitHub 커밋의 JSON 필드 **순서와 공백 차이**는 신뢰 판정에 영향을 주지 않습니다.
- **검증 명령:** `cargo fmt --all -- --check`, `cargo test --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, `python3 scripts/verify_release_contract.py`; 플랫폼별 CI 결과와 PR 체크를 함께 확인합니다.
- **릴리즈:** 변경 사항은 코드·테스트 중심이며 사용자 요청이 없으므로 새 배포 태그를 생성하지 않습니다.

## 10. 2026-11-12 G-05~G-09 계측 구현 및 잔여 판정

- G-05의 **작업자별 시드 재현성**은 결과 전체 시나리오와 원래 순서의 raw pulls를 SHA-256으로 비교합니다. 정량적 결과는 실행 조건이 다른 과거 벤치마크에서 가져오지 않습니다.
- G-06의 히스토그램 벤치마크는 **주변 확률 CDF의 동등성만** 입증합니다. 개별 시나리오 정렬의 업무 계약과 구별하며, 정렬 알고리즘 자체는 바꾸지 않습니다.
- G-07의 단계별 시간은 작업자별 독립 wall time이며, 병렬 실행 시 개별 합계가 전체 wall time과 같지는 않습니다.
- G-08은 egui CPU측 데이터 준비/텍스처 제출 시간이지 전체 GPU 렌더 지연이 아닙니다. 실제 frame p95 확보 전에는 캐시나 그래프 코드를 바꾸지 않습니다.
- G-09는 기존의 로그 이벤트마다 `flush`를 수행하는 방식과 신뢰된 경로 초기화를 유지합니다. `flush()`는 `sync_all()`과 다른 내구성 계약입니다.

상세 실행법·측정 환경 통제와 G-05~G-09 결과 판정 기준은 [docs/PERFORMANCE.md](../docs/PERFORMANCE.md)를 참조합니다.

**현재 완료 범위:** 신규 계측/벤치마크/로그 집계 코드 적용. 수치상 가속, 동적 스케줄 교체, GPU 지연 개선, 로그 기록 배치의 안전성은 실제 비교 측정과 새 계약 검증 전까지 **미완료**입니다. 신규 릴리즈는 별도 요청 전까지 진행하지 않습니다.
