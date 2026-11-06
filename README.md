# Gacha Pickup Calculator

**붕괴: 스타레일 · 명조: 워더링 웨이브 · 이환 · 스텔라 소라**의 픽업 목표, 보유 캐릭터·장비, 천장, 게임별 재화와 환급 재투자를 계산하는 **Rust/egui 데스크톱 애플리케이션**입니다.

**최신 정식 버전: v1.0.3** (2026-11-07 KST) · Windows x64 · [릴리즈와 전체 다운로드](https://github.com/P-JSL/gacha_pickup_calculator/releases/tag/v1.0.3)

[Windows 전체 ZIP](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.3/gacha-pickup-calculator-windows-x64-v1.0.3.zip) · [실제 GUI 캡처 ZIP](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.3/gacha-pickup-calculator-screenshots-v1.0.3.zip) · [SHA-256 체크섬](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.3/SHA256SUMS.txt) · [Sigstore 출처 증명](https://github.com/P-JSL/gacha_pickup_calculator/attestations/64685326)

> **정확도 안내:** 배너 규칙에 대한 픽업 확률은 유한 상태 DP로 계산합니다. 환급·중복 획득·추가 뽑기 재투입은 보유 상태를 모두 알 수 없으므로 Monte Carlo 추정치가 포함됩니다. 비공개 소프트 천장, 베타 배너, 게임 규칙 변경은 공식 결과와 달라질 수 있습니다. **분석 결과는 실제 획득을 보증하지 않습니다.**

## 시작하기

1. 위의 **Windows 전체 ZIP**을 받아 **전체 압축을 해제**합니다.
2. `gacha_pickup_calculator.exe`와 `Image/` 폴더가 같은 위치에 있도록 유지합니다.
3. `gacha_pickup_calculator.exe`를 실행합니다. 관리자 권한이나 API 키는 필요하지 않습니다.
4. 게임을 고른 다음 Dashboard 또는 Simulator에서 목표·기보유 수량을 입력하고, Currency에서 보유 뽑기권과 재화를 설정합니다.
5. 시뮬레이션을 실행하거나 자동 계산을 활성화해 Analytics의 목표 성공 확률별 **추가 필요 뽑기권 환산량**을 확인합니다.

```text
gacha-pickup-calculator-windows-x64-v1.0.3.zip
├── gacha_pickup_calculator.exe
├── Image/
│   ├── HSR/
│   ├── WutheringWaves/
│   ├── Neverness/
│   ├── StellaSora/
│   └── SHA256SUMS.txt
└── README.txt
```

**단독 EXE에는 이미지가 포함되지 않습니다.** 완전한 캐릭터·장비 이미지를 보려면 전체 ZIP을 사용하십시오. 패키지 내부의 `Image/SHA256SUMS.txt`는 PNG 검증용이고, 릴리즈의 최상위 `SHA256SUMS.txt`는 EXE·ZIP 파일 무결성 검증용입니다.

### 다운로드 파일 검증

Windows PowerShell에서 SHA-256을 확인할 수 있습니다.

```powershell
Get-FileHash .\gacha-pickup-calculator-windows-x64-v1.0.3.zip -Algorithm SHA256
```

공식 릴리즈의 체크섬과 비교한 뒤, GitHub CLI가 설치되어 있다면 **발급자 및 빌드 워크플로 출처**까지 확인하십시오.

```sh
gh attestation verify gacha-pickup-calculator-windows-x64-v1.0.3.zip \
  --repo P-JSL/gacha_pickup_calculator \
  --signer-workflow P-JSL/gacha_pickup_calculator/.github/workflows/rust.yml
```

[릴리즈 서명·운영 가이드](docs/SECURE_RELEASE_v1_0_3.md)에 사용 방법과 한계를 설명했습니다. **v1.0.3의 Windows EXE는 Authenticode 미서명 상태**입니다. Sigstore 출처 증명은 Windows 코드 서명이나 SmartScreen 신뢰를 대체하지 않습니다.

## 주요 기능

| 메뉴 | 기능 |
|---|---|
| Dashboard | 목표 부족분 KPI, 게임 선택, 캐릭터·장비별 그래프, 주요 설정 |
| Simulator | 캐릭터·장비 목표, 천장·확정 상태, 환급 및 시뮬레이션 조건 |
| Currency | 게임별 전용권, 공동 재화, 교환 포인트 및 환급 관련 입력 |
| Characters / Weapons | 캐릭터 선택·돌파·기보유, 전용 광추·무기·Arc·레코드 연결 및 재련 |
| Analytics | 50/75/80/90/95/99% 목표 성공 확률, 추가 필요 재화, 분포 비교 |
| Tracker / CSV | 현지 관측 데이터를 이용한 위험집합·검열 통계 분석 |
| Settings | 언어·차트·자동 계산과 게임 데이터·새 프로그램 버전 확인 |

- **4개 게임별 독립 상태:** 선택한 캐릭터·장비, 천장·확정 여부, 보유 티켓, 프리미엄 재화 및 환급 조건을 별도 유지합니다.
- **캐릭터 변경 시 전용 장비 자동 연동:** `src/signature_pairs.tsv`의 **141개 명시적 ID 매핑**(스타레일 66, 명조 39, 이환 11, 스텔라 소라 25)을 사용합니다. 해당 매핑이 없으면 이전 장비를 자동 해제하며, 사용자가 직접 장비를 선택할 수 있습니다.
- **미연결 항목을 임의 추론하지 않음:** 5성 미연결 **30건**의 사유는 `src/unpaired_five_star_targets.tsv`에서 별도로 관리합니다. 최신 출시·획득 가능 여부까지 모두 검증했다는 의미는 아닙니다.
- **확률 계산과 환급 추정 구분:** 별도 배너의 목표 수량·보유량·천장을 고려하며, 이미 사용한 교환 포인트의 중복 차감을 방지합니다.

> **주의:** 캐릭터 단독 부족량과 장비 단독 부족량은 각각 공용 재화를 사용한 참고치이므로 **서로 더하면 안 됩니다.** 전체 부족분은 두 목표의 동시 달성 확률을 기준으로 따로 계산합니다.

## 게임별 재화·환급 기본 가정

| 게임 | 기본 재화 / 1회 | 전용권 구분 | 교환 관련 모델 |
|---|---:|---|---|
| 붕괴: 스타레일 | 성옥 160 | 캐릭터·광추가 동일 한정 티켓 사용 | 꺼지지 않는 스타라이트 20개/티켓 |
| 명조 | 별의 소리 160 | 캐릭터·무기 구분 | 여파의 산호 8개/티켓 |
| 이환 | Annulith 160 | 캐릭터 Solid Dice / Arc Tri-Key 구분 | Warp Pieces 24개/빨간 주사위 |
| 스텔라 소라 | 별의 파편 300 | Cerulean Ticket / Gold Disc 구분 | 탐험 전문가 증명서 100개/1회 |

이 표는 **앱에 구현된 기본 계산 가정**이지, 2026년 현재 모든 지역·배너의 실제 규칙을 보증하는 목록이 아닙니다. 스텔라 소라는 5성 중복 200개·4성 중복 40개 환급 및 캐릭터 뽑기당 레코드 디스크 30개 적립을 모델링합니다. 이환의 일부 보드 반환 및 Arc 보상은 커뮤니티 추정값이므로 공식 모드와 분리해야 합니다. 자세한 가정은 [환급 보정 문서](docs/REFUND_CALIBRATION.md)에서 확인하십시오.

## 게임 데이터·이미지의 검증 범위

현재 번들에는 **679개 PNG**가 있으며, 카탈로그·매니페스트·해시·중복 여부를 검사합니다. 그러나 **모든 679개 이미지가 실제 캐릭터 외형과 정확히 일치하는지는 전수 육안 검증되지 않았습니다.** 이미지 파일의 해시 일치는 원본 매핑의 정확성을 증명하지 않습니다.

- 스타레일의 아하(게임 ID `1511`)와 광추 `23065`, `22009`는 이전 출처 대조 및 별도 해시 검증 내역이 있습니다.
- 엘렌 조·아스트라 야오에 사용된 `upcoming_ellen_joe` 및 `upcoming_astra_yao`는 검증된 공식 게임 ID가 아닌 **내부 임시 ID**입니다. 연결되지 않은 광추나 배너 규칙을 임의로 확정하지 않습니다.
- 스텔라 소라의 전용 레코드 연결은 검증 시점의 StellaDB 스냅샷과 당시 공개 자료를 참고했습니다. 향후 게임 업데이트와의 자동 일치는 보장하지 않습니다.

원본·예외·검증 한계는 [카탈로그 감사](docs/CATALOG_AUDIT.md), 게임 이미지 이용 조건은 [제3자 자산 출처](THIRD_PARTY_ASSETS.md)를 참고하십시오. 이 프로젝트는 게임사의 공식 제품이 아닙니다.

## 자동 업데이트의 범위

앱은 GitHub의 **검증된 커밋 SHA를 기준으로 게임 카탈로그·매핑을 확인**합니다. 승인된 새 PNG만 HTTPS로 다운로드하고 파일 내용·해시를 검증한 뒤 사용자 캐시에 적용할 수 있습니다. 인터넷이나 검증에 문제가 생기면 기존에 검증된 데이터 또는 번들을 사용합니다.

- **게임 데이터 업데이트:** 지원합니다. 단, GitHub 저장소에 새 데이터가 검증·반영된 이후에만 적용됩니다.
- **프로그램 버전 확인:** 최신 릴리즈를 알리지만, **EXE를 자동 다운로드·교체·실행하지 않습니다.**
- **공식 게임 업데이트 실시간 탐지:** 미지원. 게임사의 원본 캐릭터 ID·전용 장비·초상화를 자동 추론하지 않습니다.
- **독립 오프라인 서명키:** 아직 없습니다. GitHub 계정·저장소가 침해된 상황까지 완전히 방어하지는 못합니다.

**안내 정정:** 기존 v1.0.3 배포 ZIP 내부 `README.txt`의 이미지 네트워크 요청이 없다는 문구는 실제 데이터 업데이트 동작을 완전히 설명하지 못합니다. 위 내용이 실제 동작에 맞는 설명이며, 향후 패키징 워크플로에서도 문구를 바로잡았습니다. [업데이트 설계 문서](docs/AUTO_UPDATES_AND_RUST_DESIGN.md).

## 개발·검증

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo doc --locked --no-deps --lib
cargo build --locked --release
python3 scripts/verify_release_contract.py
```

이미지 수집은 일반 빌드에 포함되지 않습니다. 저장소에 포함된 `Image/` 파일을 사용하며, 외부 소스에서 이미지를 다시 받는 작업은 별도의 수동 [artwork refresh 워크플로](https://github.com/P-JSL/gacha_pickup_calculator/actions/workflows/refresh-artwork.yml)에서 검토합니다. Linux에서 이미지를 함께 실행하려면 빌드 후 `Image/` 폴더를 실행 파일 옆에 복사해야 합니다.

### 구조·성능

정식 Rust 모듈별로 확률·환급·관측 분석·데이터·UI·앱 상태를 나눴으며, 공개 라이브러리 API와 바이너리 내부 `pub(crate)` 경계를 구분합니다.

- 밀집 확률질량분포(PMF): 조건을 만족하는 경우 FFT 기반 **O(N log N)** 합성, 작은·희소 분포는 직접 계산.
- CDF 분위수 조회: **O(log N)** 이진 탐색.
- **v1.0.3 main CI 실측:** 2,048×2,048 밀집 분포 FFT **3.08배**, 분위수 20,000회 이진 검색 **52.55배** (각각 비교 대상 알고리즘 대비). 전체 GUI 성능 향상 배율은 아닙니다.

근거: [아키텍처](docs/ARCHITECTURE.md) · [성능 실측](docs/PERFORMANCE_V1_0_3.md) · [최종 CI 실행](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42300383443).

## 보안·배포 상태

| 항목 | v1.0.3 확인 상태 |
|---|---|
| Windows/Ubuntu 빌드·테스트, RustSec, 이미지 검사, GUI 캡처 | 성공 |
| CodeQL (Rust / Python / Actions) | 분석 워크플로 성공 — 발견 경고 0건을 의미하지는 않음 |
| 5개 릴리즈 자산 Sigstore 출처 증명 | 생성·서명·배포 전 검증 성공 |
| Windows Authenticode | **미서명** — 신뢰된 인증서 미설정 |
| GitHub `main` Ruleset | **미활성화** (최종 확인 시점) |
| Immutable Releases | **미활성화** (v1.0.3 `immutable: false`) |
| 계정 2FA/패스키 | 저장소 정보만으로 확인할 수 없음 |

관리자 보안 설정은 [안전한 적용 가이드](docs/SECURE_RELEASE_v1_0_3.md)에 설명했습니다. CI에 적용한 Sigstore 증명이 사용자의 PC 또는 GitHub 관리자 계정 자체의 침해를 방어하는 것은 아닙니다.

## 문서·라이선스

[변경 이력](docs/CHANGELOG.md) · [CI 변경 범위·검증 정책](docs/CI_CHANGE_SCOPES.md) · [v1.0.3 릴리즈 노트](RELEASE_NOTES_v1.0.3.md) · [보안 검토](docs/SECURITY_AUDIT.md) · [1.0 최초 출시 검토(역사적 기록)](docs/V1_RELEASE_READINESS.md) · [과거 v0.2.7 검토](docs/FOUR_PASS_AUDIT_v0.2.7.md)

애플리케이션 코드에는 [MIT 라이선스](LICENSE)가 적용됩니다. **타사 게임 이미지와 제3자 리소스의 재배포 권리는 이 라이선스로 자동 부여되지 않습니다.** 배포·재사용 시 원저작권자의 조건을 별도로 확인하십시오.
