# Gacha Pickup Calculator

Rust / `eframe` + `egui_plot` 기반 4종 게임 한정 픽업 계산기입니다.

대상 게임: **붕괴: 스타레일, 명조: 워더링 웨이브, 이환 (Neverness to Everness), 스텔라 소라**.

## 주요 기능

- 상단 드롭다운에서 게임·확률 모델·도움말 선택
- 캐릭터 E0–E6, 무기/광추 R0–R5 목표, 각 배너 이미 보유한 장수, 천장 스택 및 픽업 확정 상태 설정
- 캐릭터 전용 티켓 / 무기 전용 티켓 / 양쪽으로 교환 가능한 공통 재화 별도 입력
- **목표 달성 확률 → 최소 필요 n뽑**, **보유·입력 N뽑 → 목표 달성 확률** 양방향 조회
- 환급 재화의 교환 가격, 현재 잔액, 희귀도별·중복별 반환량과 시뮬레이션 횟수 설정
- 매 뽑기 후 반환받은 교환 포인트를 티켓으로 즉시 재사용했을 때의 **최소 초기 공용 재화** 확률분포(난수 시뮬레이션)
- 보유 재화 N으로 달성할 확률을 계산할 때 표본 오차를 위한 Wilson 95% 신뢰구간 표시
- 유한 상태 흡수 마르코프 연쇄와 정확한 동적 계획법으로 최초 목표 달성 횟수의 PMF/CDF 계산
- 두 개의 **독립된 배너를 각각 목표 획득 시 중단**하는 조건에 대한 최초 도달 시간 합성곱(컨볼루션)
- 기대 소모량, 목표 달성 분위수(50%, 75%, 80%, 90%, 95%, 99%)와 환급 전 수학적 보장 상한
- 고정 배분 대신 목표 달성 후 남는 공용권을 다른 배너에 재배분하는 정확 적응형 확률 계산
- 다크 테마 대시보드 GUI와 상단 게임/모델 드롭다운, 하단 상호작용 확률 그래프(환급 전/후 분리 표기)
- 비공식 소프트 천장 모델을 공식 기본 확률+하드 천장 모델과 분리

## 실행

```shell
cargo run --release
```

테스트 / 정적 점검:

```shell
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Windows x64 사용자는 GitHub Actions의 Windows Release 빌드 또는 `cargo run --release`로 실행할 수 있습니다. 별도 API 키, 서버, 네트워크 호출이 필요하지 않습니다.

한글 UI 폰트는 Windows의 맑은 고딕 및 Linux/macOS 시스템에 설치된 폰트를 탐색합니다. Linux 배포판에 Noto CJK가 없다면 별도로 설치해야 한글이 정상 표시될 수 있습니다.

## 산출 모델과 한계

상태 `s`는 목표 획득 수, 현재 천장, 픽업 확정 여부(게임에 따라), 스파크/배너별 진행도로 구성됩니다. 한 번 뽑을 때마다 `P_{t+1}(s') = Σ_s P_t(s) P(s→s')`를 계산하고 목표 도달 상태로 유입된 질량을 획득 시점의 확률질량함수(PMF)에 기록합니다. 계산 종료 후 두 개의 PMF를 컨볼루션하여 두 배너 목표 달성 합계의 PMF를 얻습니다.

**정확함의 의미:** 입력된 확률 모델의 규칙에 대해서는 샘플링 편차 없이 모든 경로를 합산합니다. 모델이 게임 서버의 비공개 확률식을 정확히 재현한다는 뜻은 아닙니다.

- **붕괴: 스타레일:** 캐릭터 0.6% 기본/90 하드, 픽업 50%, 실패 후 확정. 광추 0.8% 기본/80 하드, 픽업 75%, 실패 후 확정. 소프트 천장 `74회부터 +6%p`, 광추 `66회부터 +7%p`는 커뮤니티 추정이며 공식 공개식이 아닙니다.
- **명조:** 캐릭터/픽업 무기 0.8% 기본/80 하드. 캐릭터 50:50 실패 후 확정, 픽업 무기 100%. 소프트 천장 프로파일은 WutheringTools 공개 관측 모델을 기반으로 한 근사입니다.
- **이환:** 캐릭터 0.99% **기본**(종합 1.87%와 구분)/70회 실패 이후 19.59% 보드 변화, 90회 픽업 확정. Arc 무기는 10개 단위 뽑기, S급 6번째 묶음/픽업 8번째 묶음 확정, S급 중 픽업 비중 25%. Arc 모델은 10연에서 독립적으로 자연 발생한 **복수 S급을 계수**하지만, 보장 개입으로 기존 획득을 대체하는 내부 위치·순서가 공개되지 않아 보장 적용 단계는 근사합니다. 이에 따라 Arc 수치는 정확한 원게임 분포가 아닙니다.
- **스텔라 소라:** 일반 한정 캐릭터 픽업 직접 확률 1%(전체 5성 2%×50%) 및 160회 픽업 확정, 배너 누적 120회 일회성 무료 수령. 일반 디스크는 1.5%(전체 5성 2%×75%) 및 120회 픽업 확정. **비픽업 5성 획득으로 캐릭터 160회 카운트가 초기화되지 않는 규칙**을 적용합니다. 120회 보너스는 픽업 획득과 독립이며 카운트를 초기화하지 않습니다. 2.0 기념/독점 배너 등 특수 배너는 적용 범위를 별도로 확인해야 합니다.

## 재화 피드백(환급)

원시 소모의 **확률분포는 결정론적 동적 계획법(정확)**, 환급·재투입의 **필요 초기 보유량은 별도 Monte Carlo 추정**입니다. 4성의 실제 보유/중복 판정은 계정마다 달라서 기본 환급률을 사용자가 조정해야 합니다. 스타레일·명조는 공개된 참고 교환 규칙으로 초기값을 채우며, 이환·스텔라 소라는 검증되지 않은 환급 규칙을 임의로 넣지 않고 수동 입력 시에만 켜집니다.

1. 캐릭터 목표를 먼저 뽑고 무기 목표를 이후에 뽑습니다. **각 뽑기의 대금을 먼저 지불한 뒤에** 해당 뽑기로 얻은 환급 포인트를 계정에 적립합니다.
2. 반환 포인트를 교환 가격의 정수배만큼 자유 배분 티켓으로 교환합니다. 캐릭터 전용/무기 전용 티켓은 각각 먼저 소비하며, 남는 티켓은 이전할 수 없습니다.
3. 임의의 경로에서 처음 필요했던 최소 공용권 수는 매 시점의 미충당 공용권 누적 지출액의 최대값입니다. 이 값의 여러 경로에 대한 분위수가 '환급 포함 필요 n뽑'입니다.
4. 현재 설정한 4성 기본 획득률과 10회 보장 규칙을 사용하는 모의실험은 정확한 아이템 소유/최고 등급 겹침 순서를 재현하지 못합니다. 결과에는 95% Wilson 구간을 표시하며 **100% 보장**이라고 표시하지 않습니다. 이환의 Arc 10연 내부 보장 개입 시점도 근사입니다.

**명시된 한계:** 게임 재화 교환 월간 제한, 서로 다른 4성 보유량, 픽업/일반 4성 상세 분포, 이환 Arc 10연 개입 내부 순서, 이벤트·상점 구매로 무기를 확정 수급하는 전략은 반영하지 않습니다.

## 참고 출처

- Hou, S., Zhu, Y., Zhang, S. (2026), *State-Dependent Asymmetry in Soft-Pity Gacha Waiting-Time Models*, **Symmetry 18(6), 1051**. https://doi.org/10.3390/sym18061051
- HoYoverse 지원센터: https://support.hoyoverse.com/hc/en-us/articles/50913708153625-How-does-the-guarantee-system-work-for-the-Light-Cone-Event-Warp-and-does-my-pity-carry-over
- Honkai Star Rail 가챠 상세: https://www.prydwen.gg/star-rail/guides/gacha-system/
- Wuthering Waves Convene: https://wutheringwaves.gg/convene-details/
- WutheringTools 커뮤니티 실측: https://www.wutheringtools.com/convene
- Neverness to Everness 규칙: https://www.prydwen.gg/neverness-to-everness/guides/gacha-system
- NTE Arc 가챠: https://gamewith.net/nte/74412
- Stella Sora 공식: https://stellasora.global/news/4926
- Stella Sora 배너 안내: https://stellasora.pro/pity/

## 수학 검증 근거

- Hou, S., Zhu, Y., Zhang, S. (2026), *State-Dependent Asymmetry in Soft-Pity Gacha Waiting-Time Models*, **Symmetry 18(6), 1051**. https://doi.org/10.3390/sym18061051
- Sanketi, A. (2026), *Structural Convergence in Gacha Mechanism Design*, SSRN, https://papers.ssrn.com/sol3/Delivery.cfm/7162298.pdf?abstractid=7162298&mirid=1
- 붕괴: 스타레일 기본·종합·하드 천장 수치: https://www.prydwen.gg/star-rail/guides/gacha-system/
- 명조 산호 환급: https://wutheringwaves.gg/convene-details/
- 이환 기본/종합 구분: https://alkapa.gg/neverness-to-everness/guide/reroll

## 기여 / 업데이트

게임 패치로 배너 규칙이 변경될 수 있습니다. 확률 상수나 천장을 수정할 때는 원문, 게시 일자, 변경 전후 모델을 검토하고 단위 테스트를 함께 갱신하세요.