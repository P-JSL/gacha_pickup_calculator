# v0.2.1 카탈로그 정합성 검토 및 게임 데이터 교차검증

검증 일자: 2026-11-06 (KST). 기준 저장소: `P-JSL/gacha_pickup_calculator`.

> **main 개발판 업데이트 (v0.2.1 이후):** 공식 4.7 예고와 [Nanoka](https://hsr.nanoka.cc/lightcone/23065) 및 [Gachabase](https://hsr.gachabase.net/lightcones/23065/upon-the-first-echo-of-aha/beta?lang=ko)를 통해 신규 광추 `23065`와 `22009`를 교차 확인했습니다. 아하의 실제 게임 ID `1511`을 Nanoka/Gachabase 베타 데이터로 확인하여 임시 ID 대신 적용했으며 전용 광추 `23065`로 연결했습니다. 공식 4.8 공개 캐릭터 엘렌 조·아스트라 야오는 게임 내 ID, 배너 조건, 전용 광추가 확인되지 않아 `upcoming_ellen_joe`, `upcoming_astra_yao`라는 **프로그램 내부 임시 식별자**로 등록했습니다. 카탈로그/내장 이미지 679건, 명시적 자동 연결 141건(스타레일 66건), 스타레일 광추 172건입니다. 신규 항목 4건의 초기 상태는 식별용 플레이스홀더였습니다. 이번 보완에서 Gachabase의 베타 ID 1511(아하 캐릭터 Square Icon), 22009(4★ 광추 Large Icon), 23065(5★ 광추 Large Icon)를 **각각 해당 ID의 갤러리 이미지로** 확인해 저장소 내 PNG를 교체하는 검증 자동화를 추가했습니다. 두 4.8 콜라보 캐릭터는 검증된 그림이 없어 계속 플레이스홀더로 표시합니다. PNG 수집이 실제 완료되었는지는 별도 GitHub Actions 검증 결과를 확인해야 합니다. 아래 표는 기존 v0.2.1 당시 수치를 기록한 것입니다.

## 확인 결과

| 게임 | 보조 장비 카탈로그 | 자동 연결된 5★ 캐릭터–장비 | 교차검증 원본 |
|---|---:|---:|---|
| 붕괴: 스타레일 | 광추 170 | 50 | [StarRailRes 한국어 게임 데이터](https://github.com/Mar-7th/StarRailRes/tree/dbe8cdfcfb0bf657b9fe1cf92d5537f118ccb487/index_min/kr) (`source_snapshot_hsr.tsv`) |
| 명조: 워더링 웨이브 | 무기 124 | 39 | [wuthering.th.gl 무기 데이터](https://wuthering.th.gl/db/weapons), [WutheringPity 2026-10-26 전체 목록](https://wutheringpity.com/wuthering-waves/wuwa-weapon-list/) |
| 이환 | 아크 51 | 11 | [NTE_Assets 내부 ForkItemData](https://github.com/Waifus-Grace/NTE_Assets/blob/1816e237ffe9edf687dd6942d028e89cce121381/DataTable/Fork/DT_ForkItemData.json) (`source_snapshot_nte_arcs.tsv`) |
| 스텔라 소라 | 레코드 104 | 25 | [sstoy ID·한국어 지역화 데이터](https://github.com/JforPlay/sstoy/tree/33268f4a89cd314fff385a6ffdd6ecbbdc995a8b/public/data), [공식 공지](https://stellasora.global/news) (`source_snapshot_sora.tsv`) |

**현재 카탈로그 총 675개**: 원래의 677개에서 스텔라 소라의 허구적 별칭 ID인 `firstlight_noya`, `guiding_star_disc` 2개를 제거했습니다. 실제 `Firstlight Noya`는 캐릭터 `162`(`세이나(기원)`), `My Guiding Star`는 레코드 `214066`(`앞길을 밝히는 샛별`)입니다. 같은 대상을 서로 다른 ID로 중복 선택하지 않습니다.

### 실제 게임 데이터 검증 수준

- **스타레일**: 외부 데이터의 실제 캐릭터 ID 98건 + 광추 ID 170건을 고정 스냅샷으로 검증합니다. 4성 삼칠이 각 운명의 길 버전 2건과 5성 개척자 10건은 구분을 위해 이름만 주석화했습니다. `upcoming_aeon_aha`는 고정 원본에 **없으므로 미확인 발표/추가 데이터 항목**입니다. 그 존재 또는 배너 획득 가능성은 스냅샷으로 보장하지 않습니다. 전체 광추 ID·이름·희귀도는 이 게임 데이터와 일치했습니다.
- **명조**: 124개 무기는 두 독립 팬 데이터베이스에서도 목록 수 124개로 일치하지만, 전체 이전 버전의 독립 ID 목록 `yuhkix/wuwa-ids`에서 겹치는 **90개 ID와 영문 이름이 전부 일치**합니다 (`source_snapshot_wuwa_weapons.tsv`). 새 버전에 추가된 나머지 34개는 이 구버전 자료에 없어 해당 수준의 ID 검증을 완료하지 않았습니다. 특히 미출시·유출 항목은 일반 픽업 장비로 단정하지 않습니다. 현재 39개의 캐릭터 전용 무기 연결은 기존 명시적 목록을 유지합니다.
- **이환**: 내부 게임 파일의 `ItemIcon.AssetPathName`, `ItemQuality`와 전수 비교하여 아크 **51/51**이 원본 식별자와 희귀도가 일치합니다. 각 아크의 `ExclusiveCharacterIDArray`로 현재의 전용 연결 **11/11**을 확인했습니다. 참고: 게임 데이터에는 동일 현지화 이름을 가진 별도 `「제로」` ID `1051`, 동일 아이콘을 참조하는 캐릭터 `1056`도 존재합니다. 두 항목은 별도 초상화/픽업 구분을 완료하기 전까지 일반 선택으로 복원하지 않았습니다. 다른 형태의 캐릭터를 임의로 합치거나 허위 이미지로 교체하지 않습니다.
- **스텔라 소라**: 실제 등록된 캐릭터 41건/레코드 104건의 ID, 한국어 명칭 및 `Available/Visible` 값은 `sstoy`의 원본과 일치합니다. StellaDB의 [33개 캐릭터/레코드 배너 기록](https://github.com/AutumnVN/ss-data/blob/4a44bdef2fa94f8dc3db65b1d790d38783072bb8/gacha.json)을 같은 모집 기간·대응 배너 ID로 결합하여 중복 재출시를 제거한 **5성 캐릭터 25명 전원과 전용 레코드 25쌍**을 확인했습니다. [StellaDB 상세 레코드](https://stelladb.pages.dev/disc/214066)의 `Signature` 관계 및 [공식 모집 공지](https://stellasora.global/news)를 대조했고, 원본 41명/104개는 `sstoy`의 한국어 명칭 및 `Available/Visible`과 일치합니다. 기존 16쌍에는 충돌 0건이었으며 누락된 9쌍만 추가했습니다. 출시되지 않은 캐릭터/레코드의 존재 여부는 공개 출처의 갱신 시점에 종속됩니다.

### 자동 선택과 미확인 처리

`src/signature_pairs.tsv`의 ID 매핑만 사용합니다. 캐릭터를 변경하면 그 캐릭터에 **명시적으로 확인된 전용 장비가 있는 경우에만** 장비를 자동 변경합니다. 그 외에는 마지막 수동 장비 선택값을 유지합니다. 수동 장비 선택은 캐릭터 자체를 변경하지 않습니다. 연결 수치는 게임 규칙이나 확률식 자체를 변경하지 않습니다.

캐릭터가 일반 캐릭터 목록에 있어도 한정 배너 대상이 아니라면 모델상 선택을 차단합니다. 데이터에 존재한다는 사실은 전용 픽업 배너, 현재 지역 출시, 실제 획득 가능성의 증거가 아닙니다.

### 오프라인 검증

`scripts/verify_embedded_assets.py`는 외부 HTTP 요청 없이 세 게임의 스냅샷과 카탈로그/이미지 체크섬을 비교합니다. `Image/` 디렉터리의 모든 PNG는 `Image/SHA256SUMS.txt` 기준, 실제 앱에서는 컴파일된 매니페스트 기준으로 검사합니다. 출처 기록은 이미지 다운로드를 앱 실행 시 활성화하지 않습니다.

새로운 패치/유출/신규 배너 정보는 **독립 검증 후**, 현재와 다른 공개/예정/유출 상태를 분리하여 카탈로그를 추가해야 합니다. 누락된 전용 관계를 이름 유사도나 모델의 일반 5성 무기에서 추측하지 않습니다.

## 데이터 권리 및 주의

외부 원본에 게임 데이터와 아이콘이 있어도 저작권상 재배포 허가를 자동으로 부여하지 않습니다. 상세한 출처 및 라이선스 한계는 [THIRD_PARTY_ASSETS.md](../THIRD_PARTY_ASSETS.md)를 확인하십시오.
