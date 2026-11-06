# v0.2.2 캐릭터·장비·이미지 정합성 검토

기준: `P-JSL/gacha_pickup_calculator` main · 2026-11-06.

## 총괄

| 게임 | 연결된 5★ 캐릭터–전용 장비 | 명시적 미연결 예외 | 원본 대조 |
|---|---:|---:|---|
| 붕괴: 스타레일 | 66 | 12 | `source_snapshot_hsr.tsv`, 별도 4.7 공개 데이터, Nanoka/Gachabase |
| 명조 | 39 | 9 | `source_snapshot_wuwa_weapons.tsv` (90개 외부 ID·이름) |
| 이환 | 11 | 9 | `source_snapshot_nte_arcs.tsv`의 전용 아크 관계 |
| 스텔라 소라 | 25 | 0 | `source_snapshot_sora.tsv`, StellaDB 및 공식 모집 정보 |
| **합계** | **141** | **30** | |

카탈로그/로컬 이미지 총 **679건**. 5★ 캐릭터는 총 171건으로, 이 중 141건에 명시적 전용 장비가 연결되어 있으며 30건은 미연결 사유가 따로 관리됩니다(`src/unpaired_five_star_targets.tsv`). 이 숫자는 모든 게임의 잠재적 신규 캐릭터를 망라한다는 뜻이 아니라 **현재 등록된 카탈로그 기준**입니다.

## v0.2.2에서 보완한 자동 선택

캐릭터 변경 이벤트는 `apply_catalog_choice`를 통해 `src/signature_pairs.tsv`의 `(게임, 캐릭터 ID) → 장비 ID` 관계를 조회합니다. 확인된 전용 장비가 있으면 장비 선택값을 교체하며, 없으면 예전 캐릭터의 장비가 남지 않도록 선택을 비웁니다. 사용자가 장비를 직접 지정할 수 있는 기능은 유지합니다.

미연결 30건의 사유는 스토리 지급 주인공 14, 일반 상시 캐릭터 5, 이환 전용 아크 근거 미확인 7, 공개 후 전용 장비·배너 미확정 4건입니다. **신규 캐릭터를 연결표와 예외 목록 어느 쪽에도 등록하지 않으면 CI가 실패하도록 검증합니다.**

## 4.7 아하와 두 광추의 이미지

| 역할 | 게임 데이터 ID | 명칭 | 원본 대응 이미지 |
|---|---|---|---|
| 캐릭터 | `1511` | 에이언즈★아하 | `Image/HSR/c-1511.png` |
| 5★ 전용 광추 | `23065` | 「아하」가 처음 울리는 순간 | `Image/HSR/w-23065.png` |
| 4★ 광추 | `22009` | 세상 끝에서 만나자! | `Image/HSR/w-22009.png` |

각 이미지의 URL은 `scripts/pinned_icon_sources.py`에 **게임·종류·ID 단위로 고정**되어 있으며, 아이콘 수집 작업에서는 다른 임의 URL을 받지 않습니다. 수집·PNG 정규화 및 개별 SHA-256 생성은 검증된 GitHub Actions 실행에서 완료됐고, `Image/SHA256SUMS.txt` 및 별도 고정 해시로 확인합니다. 앱 실행 중 외부 이미지를 다운로드하지 않습니다.

Gachabase 게임 데이터: [아하](https://hsr.gachabase.net/characters/1511/aeon-aha/beta), [4★ 광추](https://hsr.gachabase.net/lightcones/22009/see-you-at-the-worlds-end/beta?lang=en), [5★ 광추](https://hsr.gachabase.net/lightcones/23065/upon-the-first-echo-of-aha/beta?lang=ko). 공개 정보에는 정식 출시 전 변동 가능성이 있습니다.

## 검증의 한계

- 신규 이미지 3건은 개별 게임 ID 대응을 확인했습니다. **나머지 676개 이미지의 캐릭터·장비 외형을 모두 육안으로 검증했다고 볼 수는 없습니다.** 체크섬 일치는 사진이 게임 내 정답임을 증명하지 않습니다.
- `src/source_snapshot_hsr.tsv`는 기존 고정된 `Mar-7th/StarRailRes` 자료 및 추가 4.7 베타 대조 항목을 포함합니다. 원본이 모든 미래 출시 정보에 최신이라는 뜻은 아닙니다.
- 스텔라 소라 25쌍은 `sstoy` 및 [StellaDB](https://stelladb.pages.dev/)의 모집 기록/전용 관계를 비교했습니다. 이환 11쌍은 내부 전용 아크 배열과 일치합니다. 다른 게임의 일부 향후 아이템은 버전 변경에 따라 변동될 수 있습니다.
- 4.8 공개 캐릭터 엘렌 조·아스트라 야오는 `upcoming_ellen_joe`, `upcoming_astra_yao`라는 **실제 게임 ID가 아닌 앱 내부 임시 ID**입니다. 전용 5★ 광추 및 배너 규칙이 공식 확인되지 않아 연결하지 않습니다. 타 캐릭터의 이미지를 대용하지 않고 플레이스홀더를 표시합니다.
- `src/catalog_data.tsv`의 비한정·이벤트·유출 캐릭터 수록 여부만으로 현재 가챠 획득 가능성을 확정하지 않습니다.

## 지속 확인

`scripts/verify_embedded_assets.py`는 카탈로그 679건, 연결 141건, 미연결 예외 30건, PNG 크기/형식/해시, 4.7 최신 이미지 고정 해시, 소스 및 중복을 검사합니다. 실제 Linux GUI 캡처는 `scripts/capture_readme_screenshots.sh`에서 수행하며 해당 CI 결과를 반드시 별도로 확인해야 합니다.

이 문서는 이전 v0.2.1 보고서를 대체합니다. 그 버전의 검증 기록은 [GitHub v0.2.1 릴리즈](https://github.com/P-JSL/gacha_pickup_calculator/releases/tag/v0.2.1)에 보존됩니다.
