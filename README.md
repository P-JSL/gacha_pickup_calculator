# Gacha Pickup Calculator

**현재 버전: v1.0.15** · Windows x64 · 붕괴: 스타레일 / 명조 / 스텔라 소라 / 이환

캐릭터와 전용 장비를 목표 수량까지 획득하는 데 필요한 뽑기권·재화, 목표 달성 확률, **환급 재투입 후 성공률**을 계산하는 데스크톱 프로그램입니다.

## 다운로드 및 설치

- [v1.0.15 전체 패키지(Windows x64 ZIP)](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.15/gacha-pickup-calculator-windows-x64-v1.0.15.zip)
- [v1.0.15 실행 파일(EXE)](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.15/gacha_pickup_calculator.exe)
- [실행 화면 미리보기](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.15/gacha-pickup-calculator-screenshots-v1.0.15.zip)
- [최신 배포 및 변경 사항](https://github.com/P-JSL/gacha_pickup_calculator/releases/latest)

1. **전체 패키지 ZIP**을 내려받아 압축을 해제합니다. EXE와 `Image` 폴더를 같은 위치에 둡니다.
2. `gacha_pickup_calculator.exe`를 실행합니다. 별도의 API 키나 관리자 권한은 필요하지 않습니다.
3. 게임·목표 캐릭터/장비와 목표 수량, 현재 천장/픽업 확정 상태, 보유 뽑기권과 재화를 입력합니다.
4. 입력을 모두 마친 후 **Run Simulation**을 클릭합니다. 입력 도중에는 자동 재계산하지 않습니다.
5. 상단 **환급 포함 전체 목표 달성 확률** 그래프와 예상 추가 필요 재화를 확인합니다. 캐릭터·장비별 그래프는 환급 전 참고값입니다.

**배포용 이미지 경량화:** 679개 이미지의 실제 PNG 합계는 16.69 MB에서 2.87 MB로 **82.8% 감소**했습니다. 파일명과 이미지 식별자는 유지하며, 화면 로딩 시 낮아진 해상도를 Lanczos3로 확대하고 약하게 선명도를 보정합니다. 손실된 원본 디테일을 복원하는 AI 업스케일링은 아니며, 기존 이미지 원본은 저장소에 보존됩니다.

## 계산에 포함되는 항목

| 항목 | 설명 |
|---|---|
| 게임별 뽑기 규칙 | 5성 기본 확률, 천장, 픽업 확정, 목표 중복 획득 |
| 보유 자원 | 전용 뽑기권, 게임 재화, 교환 포인트 및 잔여 단위 |
| 환급 재투입 | 획득 환급을 재사용해 최종 성공률 계산 · 스타레일은 보유권 성공 / 환급 추가 성공 / 실패 3단계 표시 |
| 4성/A급 최대 돌파 | **기본값은 모든 4성 캐릭터/A급의 최대 돌파 보유**이며 고급 설정에서 변경 가능 |
| 시뮬레이션 결과 | 전체 목표 성공 확률, 필요 재화의 구간·분위수, 추정 오차 |

**주의:** 스타레일·명조의 비픽업 4성 캐릭터/장비 구성, 소프트 천장 일부 및 이환 보드 보상 등은 근사 모델입니다. **환급률을 고정 비율로 일괄 적용하지 않고**, 획득 결과별 환급을 시뮬레이션합니다. 결과는 기대값/확률이며 실제 획득을 보장하지 않습니다.

## 업데이트와 파일 검증

프로그램은 새 버전이 있으면 알려주지만 **EXE를 자동으로 교체하지 않습니다.** 게임 카탈로그·이미지는 검증된 GitHub 데이터가 있을 때 별도로 갱신될 수 있습니다. 오프라인에서는 기존 데이터로 동작합니다.

다운로드 검증용 [SHA-256 목록](https://github.com/P-JSL/gacha_pickup_calculator/releases/download/v1.0.15/SHA256SUMS.txt)을 제공하며, Windows PowerShell에서 `Get-FileHash .\gacha-pickup-calculator-windows-x64-v1.0.15.zip -Algorithm SHA256` 명령으로 비교할 수 있습니다.

문제 제보 및 기능 요청은 [GitHub Issues](https://github.com/P-JSL/gacha_pickup_calculator/issues)에 남길 수 있습니다.

배포된 소프트웨어는 게임사의 공식 서비스가 아닙니다. 소스 코드는 MIT 라이선스이며, 게임 이미지·제3자 자료의 권리는 원저작권자의 조건을 따릅니다.
