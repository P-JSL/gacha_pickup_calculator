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
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 동일 입력·시드의 결과와 재화 흐름이 바뀌지 않고 신규 회귀가 통과
- **중단·원복:** 확률·교환 결과가 바뀌면 관련 최적화를 중단
- **기존 보고서 연결:** Q-G01, Q-G03

### G-02 · 연속 실행·취소 시 오래된 결과 차단 검증 (P0)

- **판정:** 기존 세대 보호의 회귀 확장
- **수정·확인 파일:** [`src/app/controller_session/calculation.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/controller_session/calculation.rs), [`src/app/compute.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/compute.rs), [`src/app/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/tests.rs)
- **구현 지시:** 실행→입력 수정→즉시 재실행→취소→다시 실행을 반복하는 테스트를 작성합니다. 세대번호 증가와 취소 플래그를 보존하고 이전 worker가 최신 UI/로그 성공 결과를 덮어쓰지 않게 확인합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 마지막 세대만 결과를 게시하고 중단된 작업의 오류가 현재 세대에 표시되지 않음
- **중단·원복:** 세대 가드·취소 체크 제거 금지
- **기존 보고서 연결:** IMP-G06, Q-G02

### G-03 · 원격 카탈로그 갱신의 실패·복구 경계 (P1)

- **판정:** 새 실패 주입 검증
- **수정·확인 파일:** [`src/data/runtime_updates.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/runtime_updates.rs), [`src/data/runtime_updates/cache.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/runtime_updates/cache.rs), [`src/data/runtime_updates/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/runtime_updates/tests.rs)
- **구현 지시:** 서명 실패, 허용되지 않은 URL, 중단된 다운로드, 손상 캐시, 검증 직전 프로세스 종료를 조합합니다. 이미 검증된 카탈로그를 유지하거나 명시적으로 실패하도록 테스트를 작성하고 서명 검증을 우회하는 캐시 최적화를 금지합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 검증되지 않은 번들이 활성화되지 않고 기존 정상 데이터가 오염되지 않음
- **중단·원복:** 커밋 SHA 존재만으로 신뢰 판정 금지
- **기존 보고서 연결:** Q-G04, Q-G07

### G-04 · 이미지 무결성·캐릭터와 장비 연결 정합성 (P1)

- **판정:** 카탈로그-화면 교차 검사
- **수정·확인 파일:** [`src/data/security.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/security.rs), [`src/data/catalog.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/catalog.rs), [`src/data/catalog/tests.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/catalog/tests.rs)
- **구현 지시:** 내장 SHA 매니페스트 검증을 유지하면서 캐릭터 선택 시 전용 장비 자동 연동이 틀리거나 누락된 관계를 데이터 기반으로 검사합니다. 이미지 경로 변조/동일 크기 위조/미등록 관계를 테스트합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 잘못된 캐릭터-장비 관계 및 변조 이미지가 성공으로 표시되지 않음
- **중단·원복:** 확인되지 않은 최신 게임 데이터를 임의 삽입하지 않음
- **기존 보고서 연결:** Q-G05, Q-G08

### G-05 · 성능 기준선과 확률 정확도 증거 분리 (P1)

- **판정:** 계측 먼저
- **수정·확인 파일:** [`src/domain/feedback/runner.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/runner.rs), [`tests/performance_regression.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/tests/performance_regression.rs), [`docs/PERFORMANCE.md`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/docs/PERFORMANCE.md)
- **구현 지시:** 100k/1M 표본, 게임별 캐릭터·장비, 1/2/4/8 CPU 작업자를 조건으로 추첨/정렬/집계/GUI 시간을 분리 계측합니다. 동일 하드웨어·시드·빌드·표본 수의 중앙값·p95·최대 RSS와 결과 검증을 별도로 기록합니다.
- **검증 명령:** `cargo test --locked --release --test performance_regression -- --ignored --nocapture`
- **완료 조건:** 기준선과 변경본 결과 SHA·실측 환경이 남아 있으며 과거 국소 가속 배수를 전체 가속이라고 표기하지 않음
- **중단·원복:** 프로파일 없이 가속 배수 확정 금지
- **기존 보고서 연결:** IMP-G01, IMP-G07

### G-06 · 정렬·예산 반복 집계의 추가 개선 여부 판단 (P2)

- **판정:** 측정 후 조건부 실험
- **수정·확인 파일:** [`src/domain/feedback/runner.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/runner.rs), [`src/domain/feedback/results.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/results.rs)
- **구현 지시:** 현재 이미 구현된 1회 정렬을 다시 제거 과제로 만들지 않습니다. 정렬이 충분히 큰 병목일 때만 제한된 티켓 범위 K의 히스토그램과 기존 비교정렬을 비교합니다. 예산 반복 조회가 많을 때만 누적합(prefix) 준비 비용을 포함한 개선을 시험합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 정렬 순서·분위수에 연결된 원본 시나리오·환급 결과 동일, 메모리 증가 상한 확인
- **중단·원복:** K가 크거나 단일 조회여서 더 느리면 적용하지 않음
- **기존 보고서 연결:** IMP-G01, IMP-G02

### G-07 · 명조 3전략 병렬 작업 분배 검토 (P2)

- **판정:** 실측 후 실험
- **수정·확인 파일:** [`src/app/compute.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/compute.rs), [`src/domain/feedback/runner.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/domain/feedback/runner.rs)
- **구현 지시:** 기존 1~8 작업자 제한과 세 전략별 CPU 예산을 유지하면서 각 전략 시간과 idle 구간을 측정합니다. 불균형이 입증될 때만 제한된 동적 chunk 크기를 비교합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 과잉 스레드 없이 입력별 출력·표본 재현성 유지, 실제 총 수행 시간 단축 확인
- **중단·원복:** 시드·난수 호출 순서 또는 취소 응답을 바꾸면 반려
- **기존 보고서 연결:** IMP-G03

### G-08 · 차트·이미지 렌더 지연 독립 최적화 (P2)

- **판정:** 프레임 프로파일링 후
- **수정·확인 파일:** [`src/ui/charts.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/ui/charts.rs), [`src/data/catalog/ui.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/data/catalog/ui.rs)
- **구현 지시:** 차트 지오메트리 구성/텍스처 업로드의 frame p95를 측정합니다. 실제 UI 병목일 때만 결과 세대·화면 너비 기준 유효기간이 정해진 캐시와 보이는 이미지 중심 텍스처 상한을 구현합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 그래프 값·레이블·이미지 관계 불변, frame p95 개선 및 캐시 상한 확인
- **중단·원복:** 메모리 누수나 다른 세대 그래프 재사용이면 원복
- **기존 보고서 연결:** IMP-G04

### G-09 · 세션 로그 flush 비용과 유실 방지 (P2)

- **판정:** 운영 계약 선행
- **수정·확인 파일:** [`src/app/session_log.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/app/session_log.rs)
- **구현 지시:** 메시지별 flush 누적시간을 계측한 후 필요한 경우에만 기록 전용 writer/제한된 배치 쓰기를 실험합니다. 실행 재시작 시 초기화, 심볼릭 링크 거부, 파일 경로 경고, 로그 창 20,000줄 제한을 유지합니다.
- **검증 명령:** `cargo test --locked --all-targets`
- **완료 조건:** 실제 누적 로그 지연이 감소하고 정상 종료의 기록 누락·순서 변경 없음
- **중단·원복:** 기록 손실을 수용하지 않는 상태에서 flush를 무조건 제거하지 않음
- **기존 보고서 연결:** IMP-G05, Q-G06

### G-10 · 키보드 접근성·릴리스 자산 검증 (P2)

- **판정:** 별도 UI·릴리스 검증
- **수정·확인 파일:** [`src/ui/dashboard.rs`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/src/ui/dashboard.rs), [`.github/workflows/rust.yml`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/.github/workflows/rust.yml), [`scripts/verify_release_contract.py`](https://github.com/P-JSL/gacha_pickup_calculator/blob/86a4733f54ce0e20466cfa2c3e27c67253264834/scripts/verify_release_contract.py)
- **구현 지시:** 키보드 전용 실행/설정/결과 열람·포커스 가시성을 점검하고, Windows 배포 ZIP/EXE 버전·카탈로그·이미지 매니페스트의 출처 및 SHA 검증을 CI 계약에 추가할 필요가 있는지 기존 workflow를 확인합니다.
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
