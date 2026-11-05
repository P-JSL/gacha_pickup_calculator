# 트래커 관측 분포 검증 안내

v0.1.5의 검증 화면은 로컬 UTF-8 CSV만 읽습니다. 로그인 토큰이나 게임 API 호출을 요구하지 않습니다.

## CSV 형식

방법 A: 개별 뽑기 구간. 각 행은 최고 등급을 마지막으로 획득한 이후 0스택에서 시작해야 합니다. event=1은 5성 획득이고 0은 그 전에 기록이 끊긴 관측입니다.

~~~csv
duration,event
2,1
3,0
4,1
~~~

방법 B: 스택별 위험집합. pity는 1부터 오름차순으로 연속되고, 현재 at_risk에서 events와 censored를 빼면 다음 스택의 at_risk와 같아야 합니다.

~~~csv
pity,at_risk,events,censored
1,3,0,0
2,3,1,0
3,2,0,1
4,1,1,0
~~~

방법 C: 완료된 5성 스택별 수만 있을 때. 중단 구간이 없어 신뢰성 검증에 부적합한 자료로 표시됩니다.

~~~csv
pity,count
1,5
2,5
~~~

CSV는 8 MiB 또는 200,000행까지 처리합니다. 캐릭터/광추/무기 등 배너 종류를 섞으면 결과가 무효입니다.

## 통계 및 위험

- 이산 위험률 h(k)=events(k)/at_risk(k)
- 우측 검열 처리 Kaplan-Meier 생존함수 S(k)=product(1-h(i)), 누적 획득률 F(k)=1-S(k)
- Greenwood 근사 분산 Var[S] = S^2 * sum d/[n(n-d)]. 그래프의 신뢰구간은 로그-로그 변환 근사치입니다.
- 최초 스택이 불명인 경우, 게임 계정의 이력 일부가 유실된 경우, 특정 결과에서만 업로드가 발생한 경우, 일반/한정 배너 혼합은 별도로 해결할 수 없습니다.
- 데이터의 마지막 관측 스택까지만 제한 평균 대기시간을 비교하며, 충분한 위험집합(20개 이상)이 있는 구간만 모델 편차 계산에 사용합니다.
- 이환 Arc의 10연 이중 천장과 스텔라 소라의 별도 픽업/보너스 획득은 일반 5성 천장과 직접 비교할 수 없습니다.
- 공개 그래프에서 이벤트 수만 보일 경우 at_risk를 추측하여 생성하지 않습니다. 실측 원시 자료가 없으면 모델 자동 보정도 하지 않습니다.

## 근거 자료

Kaplan-Meier survival estimator: https://www.ncbi.nlm.nih.gov/books/NBK610554/

Greenwood: https://www.medicine.mcgill.ca/epidemiology/hanley/c609/material/KaplanMeierEstimator.pdf

Discrete Kolmogorov-Smirnov test caution: https://doi.org/10.1093/biomet/65.1.235

HoYoverse official odds: https://support.hoyoverse.com/hc/en-us/articles/50913710696217-How-does-the-pity-system-work-for-different-banners-and-reruns

HSR: https://gacha-tracker.com/hsr/stats

Wuthering Waves: https://wuwatracker.com/ko/tracker/stats/100024

NTE: https://ntetracker.com/

Stella Sora: https://stelladb.pages.dev/

Note: Data on these sites are not guaranteed to have the same scope, completeness, or acquisition mechanism.

추가 제한: 여러 이력이 같은 계정에서 나오는 경우 관측 구간이 서로 독립적이지 않을 수 있으며, 일반 Greenwood 95% 구간은 계정별 군집 상관을 보정하지 않습니다. 위 파일에서 계정 ID를 수집하지 않기 때문에 계정 단위 부트스트랩은 아직 지원하지 않습니다.
