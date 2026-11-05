# Gacha Pickup Calculator

Rust / `eframe` + `egui_plot` 기반 4종 게임 한정 픽업 계산기입니다.

대상 게임: **붕괴: 스타레일, 명조: 워더링 웨이브, 이환 (Neverness to Everness), 스텔라 소라**.

## 주요 기능

- 상단 드롭다운에서 게임·확률 모델·도움말 선택
- 캐릭터 E0–E6, 무기/광추 R0–R5 목표, 각 배너 이미 보유한 장수, 천장 스택 및 픽업 확정 상태 설정
- 캐릭터 전용 티켓 / 무기 전용 티켓 / 양쪽으로 교환 가능한 공통 재화 별도 입력
- 유한 상태 흡수 마르코프 연쇄와 정확한 동적 계획법으로 최초 목표 달성 횟수의 PMF/CDF 계산
- 두 개의 **독립된 배너를 각각 목표 획득 시 중단**하는 조건에 대한 최초 도달 시간 합성곱(컨볼루션)
- 기대 소모량, 목표 달성 분위수(50%, 75%, 80%, 90%, 95%, 99%, 100%), 현재 보유 재화의 최적 배분 및 성공 확률
- 하단 상호작용 그래프: 캐릭터, 무기, 합산 목표 누적 확률
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
- **이환:** 캐릭터 0.99% 기본/70회 실패 이후 19.59% 보드 변화, 90회 픽업 확정. Arc 무기는 10개 단위 뽑기, S급 6번째 묶음/픽업 8번째 묶음 확정, S급 중 픽업 비중 25%. Arc 모델은 10연 내부에서 여러 S급 또는 여러 픽업이 동시에 등장하는 경우를 **최대 1개**로 근사합니다. 이에 따라 Arc 수치는 정확한 원게임 분포가 아닙니다.
- **스텔라 소라:** 일반 한정 캐릭터 픽업 직접 확률 1%(전체 5성 2%×50%) 및 160회 픽업 확정, 배너 누적 120회 일회성 무료 수령. 일반 디스크는 1.5%(전체 5성 2%×75%) 및 120회 픽업 확정. **비픽업 5성 획득으로 캐릭터 160회 카운트가 초기화되지 않는 규칙**을 적용합니다. 120회 보너스는 픽업 획득과 독립이며 카운트를 초기화하지 않습니다. 2.0 기념/독점 배너 등 특수 배너는 적용 범위를 별도로 확인해야 합니다.

## 재화 피드백(환급)

현재 모델은 **스타라이트, 산호, Expert Permit 등 뽑기 과정에서 다시 획득하는 교환 재화가 없는 원시 소비 횟수**를 정확하게 산출합니다. 4성의 중복 판정, 5성의 실제 소유 상태, 게임마다 다른 교환 가격/월간 제한을 확인하지 못한 상태에서 환급을 확정값처럼 적용하는 것을 금지했습니다. 환급을 포함한 결과는 추정치를 별도로 모델링해야 하며 본 버전의 정확 확률 곡선에는 포함하지 않습니다.

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

## 기여 / 업데이트

게임 패치로 배너 규칙이 변경될 수 있습니다. 확률 상수나 천장을 수정할 때는 원문, 게시 일자, 변경 전후 모델을 검토하고 단위 테스트를 함께 갱신하세요.