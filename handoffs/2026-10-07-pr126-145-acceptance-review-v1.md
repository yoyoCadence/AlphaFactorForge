# Handoff: 前一批 20 個 PR（#126–#145）驗收

Date: 2026-10-07（Asia/Taipei）
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr126-145-acceptance`（從最新 merged main 建立）
Preliminary reviewed baseline: `060f3d68f2dd2dd5cf9f4d016b951222de0308c8`（PR #165 合併後）
Final reviewed baseline: `0dafef5811282c5b711ae8e7b6725c8d8186046d`（PR #166 合併後）
PR: 修復 [#166](https://github.com/yoyoCadence/AlphaFactorForge/pull/166) 已合併；本報告由上列獨立文件分支交付。
Status: **Review complete** — 20 個 exact-head CI 120/120 與 merge trees 20/20 已核對；合併後整合判定完成。新增 P2 `CAMPAIGN-LISTING-001` 待修復；O3 完整 native campaign 驗收仍未完成。

## Summary

維護者在 #146–#165 驗收後要求修復、建立 PR、確認 CI 並合併，再往前驗收 #126–#145。本文件先保存可平行完成的審查與重現證據；尚未把前置結果當成合併後的最終驗收。

舊驗收中的 ledger 隔離／檢定數回退、#127 admission 身分與報告核對、code 函式白名單、campaign 同步預覽等問題，已由後續 PR 修復。這些已解決事項與本輪新確認的 Campaign 上市 metadata 缺口分開記錄。新缺口來自正式 service fetch 與 UI／backend 的整合：乾淨工作區的 BTC snapshot 綁定 `listed_from = null`，產品沒有公開的 crypto metadata 登記入口，Campaign 無法加入該 snapshot。

本草稿只新增這份 handoff。重現僅使用新建的隔離 temp workspace／registry／WebView2 profile、公開 Binance 封存與 ignored scratch；未修改使用者工作區、未讀取使用者 token／Tiingo 憑證。初稿先完成 service 重現，之後依授權實跑最小 native UI 拒絕驗證並清理，見最後追加的 Resolution。`tasks.md` 與其他文件由 root 管理，本切片未修改。

## Scope and evidence

- 對象固定為 **#126–#145，共 20 個 PR**。這次評估的是各 PR 的切片與目前整合狀態，不把原始 head 的已知缺陷列成現在仍存在的問題。
- 讀取各 merge diff、舊 batch／獨立 review、追加的 Resolution，以及目前相關 code／contracts。前置 code review 使用上述 `060f3d6`；同工作區正在進行的數值修復須在合併後另行核對。
- 從本機 Git 讀取 merge 的第二個 parent 作為 head，重新比較 head tree 與 merge tree：**20/20 相同**。root 本輪透過 GitHub 核對 20 個最終 head／check-runs，metadata 保存於 ignored `aff-o3-review/old20-pr-ci.json`；逐列檢查六項名稱、SHA、completed／success，共 **120/120**，CI links 已補下表。
- 歷史驗收及 CI 證據見 [#126–#130 review](2026-10-04-pr126-130-acceptance-review-v1.md)、[#131–#140 review](2026-10-04-pr131-140-acceptance-review-v1.md)、[FU-1 handoff](2026-10-04-fu1-ledger-evidence-snapshot-v1.md)。原文中的當時狀態保留；本文件使用後續 Resolution 判定目前狀態。
- 新 P2 有正式 service 公開 fetch 的實際輸出與該工作區的唯讀 SQLite 證據；後續已實際操作 native UI，確認 exact snapshot 可見、加入被拒且 draft 為空。preview／freeze／start／history／reopen **未執行**。

## Per-PR acceptance

「前置通過」僅指目前未找到該切片仍存在的新缺陷；全批最終結論須待 Status 的待辦完成。Head／merge 來自本機 Git並與 root 的 GitHub metadata 核對；CI links 對應各自最終 head 的六項成功檢查。

| PR | head → merge | 前置結論、已修復問題與目前限制 | Exact-head CI |
| --- | --- | --- | --- |
| [#126](https://github.com/yoyoCadence/AlphaFactorForge/pull/126) | `0e922ef` → `b5667fb` | 前置通過。P12-AUDIT R1–R5 的 split／來源／轉移／重開修正保留；後來發現的 A-R1「無新事件的隔離能被舊 registry 還原解除」已由 **#143** 的 versioned evidence snapshot 修復。未把舊 P2 重列為 open。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36572432621) |
| [#127](https://github.com/yoyoCadence/AlphaFactorForge/pull/127) | `35d02a9` → `226f043` | 整合後可接受。原 admission 的三項 P2：snapshot 身分、批次身分、walk-forward 證據與報告一致性，已由 **#128** 修復；原始合併 head 曾缺修正，不能單獨當作修復完成的 head。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36630471840) |
| [#128](https://github.com/yoyoCadence/AlphaFactorForge/pull/128) | `029e7f4` → `3cb58e7` | 前置通過。核對 snapshotId／batchId，從完整 fold evidence 重算／核對報告；#127 的三項修復及拒絕不一致的回歸仍在。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36708968263) |
| [#129](https://github.com/yoyoCadence/AlphaFactorForge/pull/129) | `e478512` → `3a92e9f` | 前置通過。先驗證再寫入，admission candidate 集合與 enqueue lineage 一致，凍結 costs／decision 同交易保存，campaign/run 表 append-only。舊 campaign 合約版本導致整個清單失敗的設計觀察已由 **#152** 逐列狀態處理。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36711799857) |
| [#130](https://github.com/yoyoCadence/AlphaFactorForge/pull/130) | `402b270` → `8269c7e` | 前置通過。owner／connected-service proxy、固定 invoke keys、requestId 冪等與 active-run 拒絕保留。A-R2 同步重預覽已由 **#145** 修復；其他 DB 非同步工作及 native 互動驗收不因此完成。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36714893574) |
| [#131](https://github.com/yoyoCadence/AlphaFactorForge/pull/131) | `161740b` → `b3f05c3` | **需修改：CAMPAIGN-LISTING-001（P2）**。正式 BTC service snapshot 的 pinned instrument 無上市起點，無公開 metadata 登記入口，UI 無法加入。既有 mock 可證明 draft revision／preview ID／ELIGIBLE 顯示流程，但完整 service-created native flow **尚未實跑**。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36725457871) |
| [#132](https://github.com/yoyoCadence/AlphaFactorForge/pull/132) | `4c08fe0` → `cd244ef` | 前置通過。檢定數有效值取 pinned／upgrade 的 max、降低拒絕、匯出／匯入升級聯集保留。舊 PR131-140-R1「事件鏈不變但 testsPerTrial 2→1」已由 **#143** 修復；P13 的 familyTests 投影仍是獨立 runtime 責任。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36793360692) |
| [#133](https://github.com/yoyoCadence/AlphaFactorForge/pull/133) | `653ebc3` → `3452033` | 前置通過作為 v1 統計骨架。嚴格解析、無偏抽樣、p 值／Holm／ppm 算術保留。v1 AR(1) 0.3 的 6.45% 失敗仍保留；最終方法由 **#140** 凍結 v2，**#153** 僅對宣告集合提供 final acceptance。不能宣稱 v1 或任意真實資料已校準。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36857304318) |
| [#134](https://github.com/yoyoCadence/AlphaFactorForge/pull/134) | `56be681` → `b5a3438` | 前置通過。原 R1「僅金額前綴一定能偵測重複 confirmation number」的錯誤保證已在該 PR 合併前更正，等額邊界測試保留；P13 必須先核對 reservation／family／序號身分再投影金額。相關 host startup 後續由 **#135** 處理。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36879266778) |
| [#135](https://github.com/yoyoCadence/AlphaFactorForge/pull/135) | `0125e6c` → `1764c48` | 前置通過。暖機係數範圍與固定 warm-up 的承諾已對齊（φ≤0.9）；late service startup 在 deadline＋grace 後仍有 cleanup 控制。原 R1／R2 在合併前修復，原 v1 97/2000、129/2000 結果未改。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37005074144) |
| [#136](https://github.com/yoyoCadence/AlphaFactorForge/pull/136) | `58e7404` → `556a397` | 前置通過，計畫文件切片。原 R1 對所有較長樣本／中間係數的過度支持聲明已在合併前限制成「最低受支持的已測長度／宣告配置」。後續 **#153** 結果仍使用同一界線。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37013969053) |
| [#137](https://github.com/yoyoCadence/AlphaFactorForge/pull/137) | `c6b339b` → `50b20e5` | 前置通過。Wilson 整數判定式、bounds、宣告驗證順序與既有 review 重推結果一致；診斷／最終驗收前綴 replay 保留。未重新跑整套 release simulation。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37018251272) |
| [#138](https://github.com/yoyoCadence/AlphaFactorForge/pull/138) | `276765a` → `13100ce` | 前置通過。原 square-free overflow／underflow 與 S1 decimal constant 規則的兩項 P2 均在合併前修復；power-of-two 正規化、無法表示的中間值拒絕、明確 constant 規則與大／小尺度回歸保留。十進位 JSON identity 另由 **#154** audit／既有 NUMERIC-JSON-002 追蹤，沒有用重生 calibration fixtures 掩蓋。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37084566037) |
| [#139](https://github.com/yoyoCadence/AlphaFactorForge/pull/139) | `4a21b52` → `e9e73e8` | 前置通過。72 個事前宣告、54 個完整診斷報告與 S2-R3 選擇已有獨立重播紀錄；本輪未重跑完整 4,000 次報告。保留診斷來源，不把一個 count 的排名差異稱成優越 power，256-bar power 仍未量測。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37117837930) |
| [#140](https://github.com/yoyoCadence/AlphaFactorForge/pull/140) | `759ecde` → `b9b63a3` | 前置通過作為 freeze/declaration 切片。固定 S2-R3、六個 final declarations 與保留 seed；後來 **#153** 依原宣告跑完 final acceptance。當前支持僅 `{256,512,1024}` bars × φ `{0,0.3}`、宣告 generator/family/B=799，不擴成連續長度／係數或 P13 runtime 完成。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37126987054) |
| [#141](https://github.com/yoyoCadence/AlphaFactorForge/pull/141) | `ac43b95` → `5b4a562` | 前置通過，#131–#140 review／反例的文件切片。歷史 R1 由 **#143** 的追加 Resolution 閉合；當時 final seed 未用／P13 blocked 的敘述是歷史狀態，後續 **#153** 結果有另行紀錄，原證據未改寫。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37177230441) |
| [#142](https://github.com/yoyoCadence/AlphaFactorForge/pull/142) | `e840749` → `5a7344c` | 前置通過，#1–#130 backward review 與工作單。D1–D7、FU-1／4／5 的補正有後續實作；FU-2／3／6／7／8／9 由 **#146–#152** 等後續 PR 修復。O1–O4 不是自動測試成功即完成，O3 的新 listing 阻斷在本文件記錄。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37194144687) |
| [#143](https://github.com/yoyoCadence/AlphaFactorForge/pull/143) | `eccc917` → `398c226` | 前置通過，關閉兩個已知 evidence rollback 反例。v2 binding 在同一 registry read transaction 比較隔離家族／衝突來源集合包含與 familyTests 高水位；owner transaction 持久寫回。舊格式精確讀取、未知／損壞新版拒絕、正常替換／多跳保留證據，失敗不覆寫最後有效 binding。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37198449202) |
| [#144](https://github.com/yoyoCadence/AlphaFactorForge/pull/144) | `8f469f8` → `b395121` | 前置通過。ES2020 相容的自身屬性檢查關閉函式 whitelist 觀察；constructor／toString／valueOf／hasOwnProperty／__proto__ 都以 unknown function 拒絕，沒有原型函式內容流入 arity 錯誤。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37198956405) |
| [#145](https://github.com/yoyoCadence/AlphaFactorForge/pull/145) | `753cde8` → `ce3ca94` | 前置通過。preview async＋spawn_blocking、逐 instrument DB lock，invoke declaration 參數與 freeze/start reverify 保留。關閉 A-R2 的同步預覽；完整原生視窗互動與 O3 flow 仍未實跑。 | [6/6](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37199505048) |

## R1 — P2：CAMPAIGN-LISTING-001，正式 BTC fetch 產生的 snapshot 無法加入 Campaign

### Trigger and impact

在全新的工作區，透過產品 service CLI 取得已閉合的 BTCUSDT 封存並指定成本 profile，fetch 能成功建立 snapshot；但自動註冊的商品上市起點為未知。Campaign UI 需要 snapshot 綁定的 instrument revision 提供確定上市時間，因此加入時會被拒絕，無法走後端預覽、凍結及啟動。

這是公開 fetch 到 Campaign authoring 的產品整合缺口。UI／backend 拒絕未知上市期間的規則符合既有 contract，沒有讓資料誤取得資格；不能以解除這項拒絕、使用首根 K 線推定上市日期或直接注入 DB 修復。

### Code and contract evidence

- `alpha-factor-forge/src-tauri/src/market/fetch.rs:86,101`：若不存在既有 revision，正式 fetch 使用 `default_crypto_instruments()` 登記 BTC／ETH。
- `alpha-factor-forge/src-tauri/src/market/registry.rs:462`：預設 crypto `listed_from: None`；ingest 不補上市時間。
- `alpha-factor-forge/src/components/CampaignPanel.tsx:117`：`listedFrom == null` 時回報「此 instrument 缺少上市起點，無法宣告」，不加入 draft。
- `alpha-factor-forge/src/services/campaignAuthoring.ts:64`：builder 同樣要求 snapshot 的 exact instrument revision 有上市起點。
- `alpha-factor-forge/src-tauri/src/research/campaign_snapshot.rs:158,166`：backend 核對宣告與 pinned revision 的上市期間相同，並要求 `listed_from` 存在且涵蓋 dataset range。手動填入任意宣告時間也不能通過。
- `alpha-factor-forge/src-tauri/src/runtime/service.rs:75` 的完整 CLI，Tauri command registration 與 runtime command surface，未提供 crypto instrument metadata 登記／更新入口。`campaign_commands.rs:64,71` 的 `list_market_instruments`／`list_market_snapshots` 只有讀取。Rust repository 函式能在測試中登記 revision，並不構成 operator 產品入口。
- [Binance adapter 文件 §8](../docs/market-source-binance-v1.md#8-已知限制與後續) 明載 `listedFrom` 未知、由來源 metadata 或人工登記填入是後續工作；[Campaign UI handoff](2026-09-30-p12d2d2-campaign-ui-v1.md) 卻以 CLI adapter 已建立的 snapshots 作為原生驗收入口。

### Controlled reproduction — 已執行

僅使用新建的 `%TEMP%/aff-o3-workspace-<runId>`、同 runId 的 registry／profile。debug service 使用 `AFF_TEST_TRIAL_REGISTRY_DIR`；該 seam 僅接受 temp root 下 `aff-*` workspace，避免接觸使用者的 production trial registry。Tiingo／Credential Manager 未使用。

```powershell
$env:AFF_TEST_TRIAL_REGISTRY_DIR = $ownedRegistry
& $copiedDebugService fetch --instrument crypto:binance:BTCUSDT --interval 1h `
  --from 2025-01-01 --to 2025-02-01 --no-rest `
  --cost-profile o3-smoke-cost-v1 --json --data-dir $ownedWorkspace
```

實際結果（2026-10-07）：

| 項目 | 結果 |
| --- | --- |
| fetch | **exit 0**，約 2.46 s；stdout JSON，stderr 空 |
| 來源 | 官方 `monthly:2025-01` 封存與 CHECKSUM；`--no-rest`，只讀取公開已閉合資料 |
| 時間單位 | microseconds → milliseconds，由 adapter 正式轉換 |
| requested range | UTC `[2025-01-01, 2025-02-01)` |
| coverage | expected／observed／matched **744/744/744**；blocking=false、events=[]、unclosedDropped=0 |
| snapshot | historical，status=`ok`，instrumentRowId=1，datasetId=1 |
| pinned instrument | 唯讀 SQLite join 確認 **`listed_from = null`、`delisted_at = null`** |
| 寫入狀態 | research_campaigns=0、discovery_runs=0；未注入／修改 DB |
| 成本 | `o3-smoke-cost-v1` **只為 smoke 標記**；不宣稱真實成本、研究資格或可交易性 |
| 程序 | bounded fetch 已退出；唯讀 Win32_Process 精確比對 copied executable，owned service process count=0 |

Snapshot ID：`c9050feba18c295d004a5e30b9c1ce76265d00ab3e283c42ac1090b5ce1d4ec8`。

Dataset hash：`dataset-content-v2:e006a1af75b7a3c641218edd74d8b09e1ff13c72a38550139bd24d11a7ee245c`。

### Recommended correction and tracking

新增有來源證據的公開 crypto metadata 登記入口，明確提供上市期間與 evidence，經既有 append-only instrument revision 流程驗證／保存；再由 fetch 建立綁定該新 revision 的 snapshot。既有 snapshot 的 pinned revision 與內容身分保留，不能原地補寫上市時間。官方歷史資料的首次觀測時間不等於上市事實。

建議驗收：在全新隔離工作區先依合法 operator 流程登記 metadata，再正式 fetch，UI 加入 exact snapshot，preview resolved 與 bar count 一致、freeze ID 等於 preview、start 後重讀／重開 admission；另保留無 listing、錯誤 evidence／revision、舊 snapshot 的拒絕回歸。

草稿撰寫時 `tasks.md` 有 P07 已知 listing 限制與 P12d／O3 未完成記錄，**沒有獨立 `CAMPAIGN-LISTING-001` task**。root 待最終驗收時新增／連結追蹤，本草稿不改 task board。此修復不在目前手動策略數值 PR 的範圍內。

## Closed findings rechecked at the preliminary baseline

| 舊 finding | 當前判定與修復 |
| --- | --- |
| PR126-130-A-R1（P2，永久隔離回退） | **#143 已修復**。隔離新增即改變 evidence snapshot；恢復舊 registry 即使 event seq／chainHead 不變也回報 `registry_evidence_rolled_back`。原反例納入正式 `r23_*` 回歸。 |
| PR131-140-R1（P3，testsPerTrial 2→1） | **#143 已修復**。有效檢定數高水位納入 covers；持久保存後恢復舊 registry 被拒，最後有效 binding 保留。 |
| #127 的 snapshot／batch／walk-forward 三項 P2 | **#128 已修復**，#129 admission/enqueue 整合延續核對，不信任僅有摘要或錯誤身分的報告。 |
| PR126-130-A-R2（P3，同步 campaign preview） | **#145 已修復**。async blocking worker 與逐 instrument mutex；完整 UI responsiveness 仍在 O3／DB-ASYNC scope。 |
| code 函式白名單只檢查 `in` | **#144 已修復**；本輪 Vitest 4.1.11 的專項測試涵蓋 inherited names 拒絕。 |
| 保存 campaign 遇合約升級整份清單失敗（設計觀察） | **#152 已修復**。逐列 valid／incompatible／corrupt，raw declarations／歷史 decisions 保留；不合法列無法啟動，backend 亦拒絕。 |
| #134 R1、#135 R1/R2、#136 R1、#138 R1/R2 | 各自在該 PR 的**最終合併 head 前修復**；原 review 的失敗探針與追加 Resolution 保留，不視為當前 open finding。 |

#143 的修復邊界仍遵守 trial-ledger §23：只保護已持久保存的觀察，不能追溯舊格式從未保存的 evidence，也不能偵測 workspace 與 registry 一起還原；不取代 P13 最終 freshness fence。這些已明示界線不列成新的缺陷。

## Verification and remaining acceptance

| 驗證 | 前置證據／最終待辦 |
| --- | --- |
| 20 個 merge/head trees | 本輪本機 Git **20/20 相同** |
| 20 個 GitHub final-head checks | root 本輪核對並保存 metadata；逐項名稱／SHA／completed／success 核對 **120/120**，CI run 見上表 |
| 前置專項 Vitest（4.1.11） | `exprInterpreter`／`candidateEnumeration`／`runnerConfigFixture` **55/55**；`strategyLibrary`／`campaignCommands` **15/15**；於數值修復前 baseline 執行 |
| 既有整合 full suite | `060f3d6` 的 #146–#165 review 已記錄 **1105 Vitest、591 Rust（1 ignored）、83 E2E**，typecheck／build／all-target check 及兩種 audit 通過；**這不是 repair merge 後的重新驗證結果** |
| 數值修復工作分支（root 提供，尚未 merge） | **1131 Vitest／600 Rust（170 library + 428 desktop + 2 service，1 ignored）**；typecheck／native build／all-target check、native 18 cases＋legacy 通過；E2E 重跑與最終 merge baseline 仍待 root 追加 |
| 新 P2 service repro | 正式 public fetch 與唯讀 pinned instrument 證據完成，見 R1 |
| O3 native UI「加入後拒絕」 | **已實跑並確認拒絕**；exact snapshot 可見、listedFrom=null、Add 回報上市起點缺漏、selectedDraftCount=0，見追加 Resolution |
| O3 preview／freeze／start／history／reopen | **全部未實跑，仍開放**；目前 listing metadata 入口缺口阻斷正常 BTC authoring |
| 完整 calibration release simulation | 本輪未重跑 #139 全部診斷或 #153 六格 final simulation；沿用原 immutable reports／既有 replay 範圍，不重選 seed／threshold |
| 最終 repair merge／本機驗證 | **Pending：root 補 repair PR、merge SHA、最終 baseline、實際 native／automated 結果與限制** |

目前 O3 的客觀阻斷是缺少可登記有證據上市資訊的產品入口；不需要以使用者 Tiingo token 或使用者既有 DB 代替。原工作單 O1／O2／O4、P13 runtime、性能與 NUMERIC-JSON-002 的剩餘研究/runtime rollout 保留各自追蹤，不因本報告完成而關閉。

## Owned evidence manifest and cleanup

本機 ignored evidence：`alpha-factor-forge/node_modules/.cache/aff-o3-review/`。

- `run.json`：完整 owned paths 與 copied debug service executable。
- `evidence-manifest.json`：短 manifest、URL、結果、SHA-256、未執行 native 標記及 cleanup 狀態。
- `fetch-a38c4502979c42a886a904ddef8d5544.json`：正式 service stdout，SHA-256 `a1e0dd4201de9314db9b82157e599c6c43a949c95b3bbf3bf63e27be90f9300a`。
- `listing-evidence.json`：`node:sqlite` readOnly query 結果，SHA-256 `fffc7e9426e72c5d4ad8f75c1bc3b7ffc275a3cb1844b4c00297f7d6c9569496`。
- copied service SHA-256：`088771db5bcd85e2f236363561220e24ad13a6f5b2be03d46f90f56daac9d67e`。重現使用既有 debug build 的隔離複本；它不是正在修改中的 numeric repair binary。
- owned runId：`a38c4502979c42a886a904ddef8d5544`；三個目錄分別為 `%TEMP%/aff-o3-workspace-<runId>`、`aff-o3-registry-<runId>`、`aff-o3-webview-<runId>`。

初稿曾暫保留三個目錄供 root 協調 native UI proof；後續最小拒絕驗證及三個目錄的清理已完成，見下方 Resolution。暫存證據不會依賴使用者資料；可攜帶的關鍵結果與 hash 已在上文保存。

清理時先由 native launcher 核實、停止它自己的 desktop／WebView2 PID，確認 service 已退出。ignored `cleanup-owned.ps1` 預設只驗證，核對每個 resolved absolute path 等於上述 temp root／精確 runId 目錄，拒絕 reparse points，唯讀查詢確認沒有引用 owned paths／copied service 的程序；查詢權限不足時停止，不把空結果當作清理證據。

```powershell
# 先唯讀驗證；Win32_Process 如遭 sandbox 拒絕存取，原命令升權重試。
& ./alpha-factor-forge/node_modules/.cache/aff-o3-review/cleanup-owned.ps1

# root 已核實並停止 native hosts 後，才執行同一 PowerShell 的 LiteralPath 刪除。
& ./alpha-factor-forge/node_modules/.cache/aff-o3-review/cleanup-owned.ps1 `
  -Execute -NativeHostsStopped
```

此方法只移除三個 owned temp 目錄，保留 ignored JSON／script 證據。最終 cleanup 實際結果已追加於下方。

## Resolution

Pending. root 完成 repair merge 後，追加最終 baseline 與整合驗證結果。20 個 final-head CI 已補齊；下面的 native 拒絕 proof 不會把「完整 Campaign flow 未執行」改成通過。

### Resolution — 2026-10-07，native 上市缺漏拒絕 proof 與 owned cleanup

root 完成數值修復的 native probe、釋放原有桌面後，授權對上述正式 service 工作區執行最小拒絕驗證。使用當前 debug desktop `src-tauri/target/debug/alpha-factor-forge.exe`，SHA-256 `b1999a0e75ad6994eb0682bf3a6900090de10a4d901047098b19333c8a8f4b16`；hidden Start-Process 啟動 PID **33524**，CDP **127.0.0.1:64924**。啟動前確認沒有其他桌面；CDP owner 必須是精確 owned profile 的 WebView2，沒有變更 machine policy。

實際 native URL 為 Tauri localhost，沒有 `?mock`，workspace mode 為 `desktop-embedded`。真正 Rust invokes 返回上述 exact snapshot、744 bars、snapshot 自身的 `qualificationEligible=true` 與 pinned revision `listedFrom=null`。UI 展開 Campaign、選取 snapshot、點 Add 後，DOM 實際顯示 **「此 instrument 缺少上市起點，無法宣告」**；`selectedDraftCount=0`，preview／freeze 按鈕 disabled，沒有 preview 元件、campaign 清單前後皆空。頁面／console errors 為空；UI 後再以唯讀 SQLite 確認 campaign／discovery run 計數仍為 0。

僅執行查詢、展開、選取、Add 與拒絕斷言；**未呼叫 preview／freeze／start，未建立 admission／history，未重新開啟 decision**。O3 完整流程仍因 R1 缺口未完成。截圖記錄 native Campaign 的 snapshot 選項與面板；精確拒絕文字／空 draft 由 JSON 中的實際 DOM 斷言保存。

新增 ignored artifacts：`native-listing-result.json`、`native-listing-rejected.png`、`native-owned-launch.json`、`native-cleanup-result.json`，以及只用於此次證據的 scratch launcher／probe。沒有修改 tracked harness 或 instrument metadata。

finally 以 executable path／CreationDate 核實 desktop PID，先保存 owned profile／descendant WebView2 身分，再停止 desktop **33524** 及 owned WebView2 **6360／36088／44364／34764**。cleanup helper 核對 resolved absolute targets 與 runId、拒絕 reparse points／存活程序後，用 PowerShell LiteralPath 移除三個 owned temp 目錄。最終 **workspaceAbsent=true、registryAbsent=true、profileAbsent=true、CDP listeners=0、cleanupError=null、proofError=null**；沒有停止其他桌面或使用者瀏覽器。

### Resolution — 2026-10-07，修復 merge 後的正式驗收

修復 PR **#166** 的 final head `a7f0783481681057ea1b545d3231edaa49021f2d`
六項 CI（typecheck/test/build/cargo-check/e2e/native-smoke）全數成功，
見 [run 37623643276](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37623643276)。
維護者授權的 merge 已於 **2026-10-07 20:56（Asia/Taipei）**完成，merge
`0dafef5811282c5b711ae8e7b6725c8d8186046d`；本機 main 已 fetch/fast-forward。
Merge tree 與已驗證的 final head tree 同為 `335bec9e77b6796d34a71bee9c469e835654bc38`。

在此合併後基準重新核對本批 ledger binding、admission identity、campaign
snapshot／command surface、函式 whitelist 及凍結統計合約；數值切片沒有改動
這些研究/runtime contracts，完整 suites／CI 證據適用同一檔案樹。
本機完整 **1131 Vitest／600 Rust（1 ignored）／85 E2E**、typecheck/build/
all-target check，以及真實 Tauri 18-case numeric＋legacy/copy/rejection smoke
已通過；fetch/rebase 後的 Vitest/typecheck/build/Rust/all-target check 也通過。
另外 fresh GitHub inline review comments 查詢確認本批 20 PR 均為 0；各
exact-head 六項名稱/SHA/completed/success 已逐項核對，並非只看 PR 綠燈摘要。

**結論：19 個切片可接受，#131 的正式來源到 Campaign 整合仍需補修 P2
CAMPAIGN-LISTING-001；無新增 P1，沒有需要 revert 的證據。** 已關閉的舊
findings 維持關閉。新 P2 已另列 `tasks.md` Backlog，沒有在數值修復 PR 中
擴大實作 metadata 功能。O3 只完成正式 fetch 與原生 Add 拒絕的最小重現，
preview/freeze/start/history/reopen 仍未執行；P12/P13 和 parent
NUMERIC-JSON-002 的剩餘 rollout 保持開放。審查完成不代表這些產品缺口已完成。
