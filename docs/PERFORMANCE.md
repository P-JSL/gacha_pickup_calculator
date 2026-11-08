# 성능 및 검증 기준

**현재 버전: v1.0.12**

## 알고리즘

- 밀집 확률질량분포(PMF): 조건에 따라 FFT 합성, `O(N log N)`. 작은·희소 분포는 직접 합성하며, 항상 FFT를 택하지는 않습니다.
- 정렬된 누적분포(CDF) 분위수: `O(log N)` 이진 탐색.
- 환급 Monte Carlo: 입력된 표본 수 N에 비례한 샘플링. 고정 시드로 작업자 수에 관계없이 재현성을 유지하고, CPU/표본 수에 따라 최대 8개 작업자를 사용합니다.
- 결과 UI는 계산 워커 종료 신호를 사용하고, 버튼 클릭 전 입력 변경으로 Monte Carlo를 자동 실행하지 않습니다.

## 재현 방법

```sh
cargo test --locked --all-targets
cargo test --locked --release --test performance_regression -- --ignored --nocapture
```

`tests/performance_regression.rs`에서 직접 합성/FFT 및 분위수 결과의 정확성을 비교하고 같은 환경에서 실행 시간을 기록합니다.

### 과거 측정의 해석

2026-11-07의 기존 CI 실험에서는 2,048×2,048 밀집 PMF의 FFT가 직접 합성 대비 약 **3.08배**, 20,000회 분위수 이진 검색이 선형 검색 대비 약 **52.55배**였습니다. 이 값은 **당시 단일 Runner의 특정 마이크로벤치마크** 결과이며, 현행 버전의 모든 환경에서 동일한 속도가 나온다는 뜻이 아닙니다. [실행 기록](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42300383443).

현행 빌드/테스트/성능/보안 게이트는 [v1.0.12 릴리즈 워크플로](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42464003186)에서 확인할 수 있습니다.
