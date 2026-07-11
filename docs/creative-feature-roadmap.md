# AlphaFactorForge — Creative Feature Roadmap

> 撰寫日期：2026-07-07。前置閱讀：[project-audit-masterplan.md](project-audit-masterplan.md)。
> 原則：**只提案強化「可信驗證」核心目標的功能**；酷但偏軸的點子集中在第 6 節（Tempting Traps）。
> 本文件為評估與規劃，未經 maintainer 挑選前，任何功能不得逕行實作。

---

## 1. Product Goal Restatement

這個專案真正應該服務的目標，不是「幫使用者找到會賺錢的策略」，而是：

> **把「我找到一個看起來會賺的策略」轉變成「我知道這個策略經得起多嚴格的檢驗、以及它在哪裡會失效」。**

換句話說，AlphaFactorForge 的產品本體是一台**信任度製造機**（trust engine）：

- 輸入：策略假說（手動參數／積木／程式碼，未來加 AI 生成 DSL）＋ frozen dataset。
- 輸出：**有紀律背書的結論**——不只是報酬數字，而是「這個數字經過了哪些檢驗、沒經過哪些」。
- 護城河：determinism、hash 去重、train/val/test 紀律、白名單 DSL——這些別的散戶工具沒有，而這裡已經在 schema 與 core 層打好地基。

因此每個創意功能的檢驗標準是同一句話：**它讓使用者對結論的「信任校準」變得更準了嗎？**（更準包括「更快看穿一個爛策略」——這往往比找到好策略更有價值。）

## 2. User Motivation Model

使用者（獨立量化研究者；目前主要是 maintainer 本人）為什麼會用、回來用、持續用：

| 動機層 | 現況 | 缺口 |
| --- | --- | --- |
| **Immediate utility** | 回測管線端到端可用、掃描熱力圖即時 | 真實資料進不來（audit Q1）；結果只有一張表，沒有曲線 |
| **Emotional reward** | 熱力圖轉綠、套用最佳的瞬間爽感 | 爽感目前來自「歷史最佳」——恰好是最該懷疑的訊號；需要把「通過嚴格檢驗」變成新的爽感來源 |
| **Progress feedback** | tasks.md 記錄工具的進步，但**策略研究本身沒有進度感** | 沒有 run 歷史、沒有「我這週淘汰了 12 個假說」的累積敘事 |
| **Habit formation** | 無回訪鉤子 | 策略庫（FEAT-001）+ 實驗日誌是最自然的「回來看看」理由 |
| **Identity building** | 「我是有紀律的量化研究者」——產品文案已在塑造（過擬合警語） | 可用評分/徽章把紀律變成看得見的身分（不做社交排名） |
| **Social / sharing** | 無 | 單人工具；分享 = 匯出好看的報告卡即可，不建平台 |
| **Reduced friction** | HelpTip、一鍵套用、樣本資料 | 資料匯入仍要手貼 JSON；策略存了拿不回 |
| **Perceived intelligence / delight** | 安全直譯器、replay 播放 | 「工具主動指出你正在騙自己」是本產品最獨特的 delight 機會 |

**結論**：最高槓桿的動機缺口是 *progress feedback*（研究歷程的累積）與 *perceived intelligence*（工具主動做信任校準）。以下功能提案圍繞這兩點。

## 3. Feature Ideas

> 依主題分組；每項含完整評估欄位。複雜度以現有架構為基準（S/M/L）。

### F1 — 權益曲線與回撤圖（Equity & Drawdown Chart）

- **User problem solved**: 一張 12 列的指標表無法回答「這策略的賺法長什麼樣？是穩定爬升還是兩根暴衝？」
- **Why it fits**: 權益曲線是信任校準的第一視覺；legacy 有、port 還沒有，屬「還債型」高價值功能。
- **Core interaction**: 回測後，績效卡上方出現 equity 線 + 水下回撤（underwater）副圖；holdout 模式畫出 in/out 分界線。
- **MVP**: 靜態 canvas 畫 `result.equity`（資料已存在於 `BacktestResult.equity`，零新資料需求）+ 回撤填色；複用 `charts/scale.ts` 純函數。
- **Advanced**: 疊 Buy & Hold 對照線、hover 讀數、與 K 線圖游標連動。
- **Data needed**: 無新資料（`equity: EquityPoint[]` 已在記憶體）。
- **Technical complexity**: **S-M**（新 canvas 元件 + scale 複用；有 CandleChart 前例）。
- **Risk**: 低。Canvas 像素不可 e2e 斷言（沿用「幾何進 scale.ts 單元測試」慣例）。
- **Abuse / privacy / maintenance**: 無。
- **Why users care**: 每次回測都看得到；是所有量化工具的預期配備。
- **Build?**: **建議做**，且應該早做——便宜、天天用、零資料依賴。

### F2 — 過擬合風險評分卡（Overfit Risk Score）

- **User problem solved**: 使用者看到綠色熱力圖與漂亮 Sharpe 就想相信；沒有任何機制主動潑冷水。
- **Why it fits**: 這就是產品使命的 UI 化身；Phase B Gate/Score 的前導最小版。
- **Core interaction**: 回測/掃描後，績效卡顯示 3-5 個紅黃綠燈：**樣本外衰退**（holdout OOS/IS 淨報酬比）、**交易數充分性**（<30 筆黃、<10 筆紅）、**參數孤島**（最佳格與鄰域格的指標落差——資料已在 sweep grid 裡）、**曝險合理性**（exposure 極端值）。每燈附一句 HelpTip 解釋。
- **MVP**: 純函數 `assessOverfitRisk(result, holdoutResult?, sweepResult?) -> RiskFlag[]` 放 `src/services/`（單元測試友善），UI 只是燈號列。**不做綜合分數**（避免假精確），只做旗標。
- **Advanced**: 納入 Monte Carlo（F6）與 benchmark 勝負（F4）；Phase B 時演化為正式 Gate。
- **Data needed**: 全部已在記憶體（metrics、holdout 對照、sweep grid）。
- **Technical complexity**: **M**（規則設計 > 程式量；規則閾值需 maintainer 簽核）。
- **Risk**: 中——閾值武斷會製造錯誤信任或狼來了；MVP 用文獻常識值並在 HelpTip 揭露規則。
- **Abuse / privacy / maintenance**: 規則要有單一檔案歸屬，避免散落。
- **Why users care**: 「工具比我更早看出我在騙自己」是這個產品能給的最獨特體驗。
- **Build?**: **建議做**（差異化核心），MVP 刻意小。

### F3 — 實驗日誌 / Run History（Research Journal）

- **User problem solved**: 每次回測結果閱後即焚（除非手動儲存）；無法回答「我上週試過什麼？哪些方向死了？」
- **Why it fits**: 反過擬合的第一課是「記住你試過幾次」——嘗試次數本身就是 data-mining 懲罰的輸入（Phase B score 已規劃此概念）。
- **Core interaction**: 每次「執行回測」自動追加一筆輕量 run log（策略 hash、參數摘要、關鍵指標、時間）；新「歷史」區塊可瀏覽、點擊還原當時策略設定；顯示「本資料集你已嘗試 N 次」計數。
- **MVP**: 沿用既有表——`strategy_def`（source 沿用）+ `backtest_summary` 已能表達大半；缺的是「未命名嘗試也記錄」的決策與 UI。最小版：**只記憶體 session 內**的 run list（零 schema 變更），關 app 即失。
- **Advanced**: 落庫（需 0002 migration 加 `run_log` 表或重用 summary+auto-name）、嘗試次數警示（「此資料集第 47 次調參，過擬合風險↑」）、週報摘要。
- **Data needed**: MVP 零新增；Advanced 需 migration（**需 maintainer 核准 schema 變更**）。
- **Technical complexity**: MVP **S-M**；Advanced **M-L**。
- **Risk**: 低（MVP）；Advanced 的 schema 決策要想清楚（見 Open Question 於 masterplan Q2 之後另議）。
- **Abuse / privacy / maintenance**: 本機資料，無隱私外洩面；日誌膨脹需清理策略。
- **Why users care**: 研究有了「存檔進度條」，回訪動機從零變成每天。
- **Build?**: **建議做 MVP**（session 內），落庫版排在 FEAT-002 之後評估。

### F4 — Benchmark Gauntlet（基準對照擂台）

- **User problem solved**: 淨報酬 +40% 聽起來很棒——直到你發現 Buy & Hold 是 +90%。目前 UI 完全沒有基準對照。
- **Why it fits**: 「必須贏過 benchmark」是 STRATEGY_DISCOVERY 的不可妥協紀律 #5；這是它的手動版預演。
- **Core interaction**: 績效表新增一欄或一列群：Buy & Hold / SMA cross / RSI 回歸 / 隨機進場（固定 seed × N 次取中位）在同資料集同成本模型下的同表指標；輸的欄位標紅。
- **MVP**: `src/services/benchmarks.ts` 純函數產生各基準的 signal series（隨機用 mulberry32 固定 seed），跑同一 `runBacktest`；UI 在 metrics 表加對照欄。
- **Advanced**: Phase B 把它變成 Gate 的一部分（自動淘汰）。
- **Data needed**: 無新資料。
- **Technical complexity**: **M**（基準定義要跟 Phase B 規格一致，避免做兩次）。
- **Risk**: 低-中；隨機基準的 N 與 seed 策略需固定以保 determinism。
- **Why users care**: 一眼看穿「其實不如躺平」，是最快的信任校準。
- **Build?**: **建議做**，但排在 F1/F2 之後（同屬績效卡改造，避免同檔連環撞）。

### F5 — 參數鄰域穩定度視覺（Plateau Finder）

- **User problem solved**: 熱力圖上「孤島最佳格」與「高原區」看起來都是綠的；使用者手動判讀穩定度。
- **Why it fits**: 「選高原不選尖峰」是反過擬合的實務鐵則。
- **Core interaction**: 掃描結果加一個切換：「穩定度視圖」——每格顯示自身與 8 鄰域的平均/最差值；或直接在原圖用邊框標出最大高原區。
- **MVP**: 純函數 `neighborhoodScore(grid) -> grid'`（S），SweepHeatmap 加顯示模式 toggle。
- **Advanced**: 「套用最穩」按鈕與 F2 的參數孤島旗標共用邏輯。
- **Data needed**: 既有 sweep grid。
- **Technical complexity**: **S-M**。
- **Risk**: 低。
- **Why users care**: 把「套用最佳」的爽感升級成「套用最穩」的正確爽感。
- **Build?**: **值得做**；可與 F2 合併規劃（共用鄰域計算）。

### F6 — Monte Carlo 重抽驗證（Trade-order Shuffle）

- **User problem solved**: 單一 equity 曲線隱藏了運氣成分；使用者無法感知「同樣的交易換個順序，回撤可能翻倍」。
- **Core interaction**: 回測後點「壓力測試」→ 對 closed trades 做 N=1000 次順序重抽（固定 seed），顯示 maxDD / 淨報酬的 5%-95% 區間帶。
- **Why it fits**: 直接強化信任校準；輸出可餵 F2。
- **MVP**: 純函數 + 直方圖/區間文字（不畫複雜圖）。
- **Advanced**: block bootstrap（保留自相關）、equity 扇形圖。
- **Data needed**: 既有 trades。
- **Technical complexity**: **M**（統計方法選擇需寫清楚假設；重抽在 worker 跑）。
- **Risk**: 中——簡單 shuffle 假設交易獨立，需在 UI 誠實揭露方法限制。
- **Why users care**: 「最大回撤其實是個分布」是多數散戶第一次看到的真相。
- **Build?**: 值得，但排 F2 之後（先有旗標框架再掛新訊號源）。

### F7 — 策略對決（Compare A/B）

- **User problem solved**: 改了參數後「有沒有比較好」全靠記憶對照。
- **Core interaction**: 策略庫（FEAT-001）中勾兩個策略 →並排 metrics 表 + （F1 完成後）雙 equity 曲線。
- **MVP**: 並排表格（複用 renderMetricsTable 的欄位機制，如 holdout 三欄的做法）。
- **Advanced**: 差異高亮、同圖疊線。
- **Data needed**: 已存 summary（讀回即可）；即時重跑亦可。
- **Technical complexity**: **M**。
- **Risk**: 低。
- **Why users care**: 迭代研究的基本動作終於不用開兩個視窗抄數字。
- **Build?**: 值得，**依賴 FEAT-001**（策略庫先存在）。

### F8 — Trade Autopsy（逐筆解剖：MAE/MFE + 最痛清單）

- **User problem solved**: 指標摘要看不出「贏靠三筆、輸靠鈍刀割肉」這類結構問題。
- **Core interaction**: 績效卡下方「交易明細」表：每筆 entry/exit/pnl/持有 bars + MAE/MFE（需回測時沿路記錄各持倉期間最高/最低價）；點列跳到圖上該區間。
- **MVP**: 表格 + 排序（pnl 最差前 10）。MAE/MFE 需要引擎在持倉迴圈多記兩個極值——**這是 core/backtest 行為擴充**，必須先過 TEST-002 golden tests 這關（新增欄位不改既有數字）。
- **Advanced**: MAE/MFE 散點圖、按出場原因分組（signal/SL/TP/eod——engine 已有 reason 字串但目前丟棄）。
- **Data needed**: 引擎補記 MAE/MFE 與 exit reason（`ClosedTrade` 加欄位）。
- **Technical complexity**: **M**（觸碰核心引擎 = 需 golden tests 護航）。
- **Risk**: 中（動 core）；緩解：欄位只增不改。
- **Why users care**: legacy 有 MAE/MFE，老用戶預期存在；是找「策略為什麼爛」的顯微鏡。
- **Build?**: 值得，排 TEST-002 之後。

### F9 — 資料集健康徽章（Dataset Health Badge）

- **User problem solved**: 匯入的資料有缺洞、重複、時間亂序時，回測結果錯得無聲無息。
- **Core interaction**: 資料集選單旁顯示徽章：跨度、根數 vs 理論根數（缺口率）、是否含未來時間戳；紅黃綠。
- **MVP**: 純函數 `assessDataset(candles, interval)`（S）+ 選單旁小徽章 + HelpTip。
- **Advanced**: 匯入時即驗證並拒絕/警告；缺口視覺化。
- **Data needed**: 既有 candles。
- **Technical complexity**: **S**。
- **Risk**: 低。
- **Why users care**: Garbage in 的防呆；真實資料入口（Q1）打開後價值倍增。
- **Build?**: **值得做**，小而美；建議與 FEAT-003（檔案匯入）同期。

### F10 — Walk-Forward Mini（滾動樣本外）

- **User problem solved**: 單次 holdout 的結論可能只是「剛好尾段行情配合」。
- **Core interaction**: holdout 區塊升級：選「滾動 K 折」→ 依時間切 K 個 (train→test) 視窗跑回測，顯示各窗 OOS 指標的一致性。
- **MVP**: 3 窗固定切分、表格輸出（不畫圖）。
- **Advanced**: embargo gap、與 sweep 結合（每窗獨立掃描＝真 walk-forward optimization——重運算，需 worker/Rust）。
- **Data needed**: 既有 candles。
- **Technical complexity**: **M**（MVP）/ **L**（advanced）。
- **Risk**: 中——與 Phase B 的正式 Train/Val/Test + embargo 設計重疊，做太深會做兩次。
- **Why users care**: 「三個窗都賺」比「一個 holdout 賺」可信得多。
- **Build?**: MVP 可做但**建議等 Phase B 設計定案**，避免拋棄式實作（見第 6 節邊界）。

### F11 — 報告圖卡匯出（Shareable Report Card）

- **User problem solved**: 想把結果貼給朋友/未來的自己，現在只有 JSON/CSV——不可讀。
- **Core interaction**: 「匯出圖卡」→ canvas 合成一張 PNG：策略摘要、關鍵指標、equity 縮圖、**強制水印「歷史回測，非投資建議 / 樣本外：{有|無}」**。
- **MVP**: 固定版型 canvas 渲染 + save_report 擴充存 .png（Rust 端白名單加 png）。
- **Advanced**: 主題色、含 F2 風險燈號。
- **Data needed**: 既有 result（F1 完成後含 equity 縮圖）。
- **Technical complexity**: **M**。
- **Risk**: 低-中；「分享績效圖」有助長 cherry-picking 的倫理面——水印與 OOS 標示為強制緩解。
- **Why users care**: 研究成果第一次「可見人」。
- **Build?**: 可做，優先級低於一切驗證類功能；**依賴 F1**。

### F12 — Replay 盲測訓練（Guess-the-next-bar Drill）

- **User problem solved**: 使用者對「訊號在當下看起來如何」缺乏體感，容易高估策略在即時情境的可執行性。
- **Core interaction**: replay 模式加「盲測」：隱藏未來、逐根播放，在訊號觸發根暫停問「跟單？跳過？」，結束後對比使用者選擇 vs 策略全跟的績效差。
- **MVP**: 沿用 replay 基礎設施 + 選擇記錄 +對比表（session 內）。
- **Advanced**: 訓練統計（你比策略多賺/少賺）、心理偏誤提示。
- **Data needed**: 既有 replay + signalSeries。
- **Technical complexity**: **M**。
- **Risk**: 低；但屬 delight 類，佔用核心開發位。
- **Why users care**: 好玩、有教育性、貼合「紀律」品牌。
- **Build?**: 好點子但**現在不做**（排創意池，等核心迴路閉環）。

## 4. Feature Scoring Matrix

> 1–5 分。**Cost 分數越高＝越容易做；Risk 分數越高＝風險越低。** Total 為等權重加總（滿分 35）。

| Feature | Strategic fit | User value | Retention | Differentiation | Impl. cost (高=易) | Tech risk (高=安全) | AI-agent suitability | **Total** |
| --- | :-: | :-: | :-: | :-: | :-: | :-: | :-: | :-: |
| F1 權益/回撤圖 | 5 | 5 | 4 | 2 | 4 | 4 | 4 | **28** |
| F2 過擬合風險燈號 | 5 | 5 | 4 | 5 | 3 | 3 | 3 | **28** |
| F3 實驗日誌 (MVP) | 5 | 4 | 5 | 4 | 4 | 4 | 4 | **30** |
| F4 Benchmark 擂台 | 5 | 4 | 3 | 4 | 3 | 4 | 3 | **26** |
| F5 鄰域穩定度 | 4 | 4 | 3 | 4 | 4 | 4 | 4 | **27** |
| F6 Monte Carlo | 4 | 4 | 3 | 4 | 3 | 3 | 3 | **24** |
| F7 策略對決 | 3 | 4 | 4 | 3 | 3 | 4 | 3 | **24** |
| F8 Trade Autopsy | 4 | 4 | 3 | 3 | 2 | 2 | 2 | **20** |
| F9 資料健康徽章 | 4 | 3 | 2 | 3 | 5 | 5 | 5 | **27** |
| F10 Walk-forward mini | 5 | 4 | 3 | 4 | 2 | 2 | 2 | **22** |
| F11 報告圖卡 | 2 | 3 | 2 | 3 | 3 | 4 | 3 | **20** |
| F12 盲測訓練 | 3 | 3 | 4 | 5 | 3 | 4 | 3 | **25** |

> 注：AI-agent suitability 低分（F8/F10）＝需要動 core 引擎或與 Phase B 設計糾纏，不適合便宜 agent 獨立執行。

## 5. Recommended Creative Features（Top 3）

### 🥇 F1 — 權益曲線與回撤圖

- **Why this one**: 最高「日常使用價值 ÷ 成本」比；資料已在記憶體；是 F2/F6/F7/F11 的視覺地基。
- **Why now**: 與 REF-003 完成後的 ResultsSection 自然銜接；不依賴任何 Open Question。
- **MVP scope**: `EquityChart.tsx` 靜態 canvas（equity 線 + underwater 填色 + holdout 分界虛線）；幾何函數進 `scale.ts` 或新 `equityScale.ts` 並單元測試。
- **Phase plan**: P1 靜態圖 → P2 Buy&Hold 疊線（依賴 F4 的 benchmark service）→ P3 hover 連動。
- **Validate first**: 無（零資料依賴、零決策依賴）。
- **Not yet**: 不做縮放/平移、不做多策略疊線。
- **Suggested backlog tasks**: `FEAT-004 equity chart MVP`（S-M；照 REF 後的元件慣例新開 section 檔）。

### 🥈 F2 — 過擬合風險燈號（+ F5 鄰域穩定度合併規劃）

- **Why this one**: 全表最高差異化；把產品使命變成每次回測都看得到的介面元素；為 Phase B Gate 累積規則資產。
- **Why now**: BUG-001 修正後，holdout 對照數字才乾淨，燈號才有意義（**依賴 BUG-001 先合併**）。
- **MVP scope**: `src/services/riskFlags.ts` 純函數（4 個旗標：OOS 衰退、交易數、參數孤島、極端曝險）+ ResultsSection 燈號列 + HelpTip 揭露規則；**不出綜合分數**。
- **Phase plan**: P1 四旗標 → P2 併入 F5 鄰域計算與「套用最穩」→ P3 接 F4/F6 訊號源 → Phase B 演化為 Gate。
- **Validate first**: 閾值表（例：OOS/IS < 0.5 = 紅）需 maintainer 簽核一頁規則文件後才實作。
- **Not yet**: 不做自動淘汰、不做分數排序。
- **Suggested backlog tasks**: `FEAT-005a 規則文件`（S，純文件）→ `FEAT-005b riskFlags service + tests`（S）→ `FEAT-005c 燈號 UI + e2e`（S）。

### 🥉 F3 — 實驗日誌 MVP（session 內 Run History）

- **Why this one**: 最高 retention 分；把「嘗試次數」這個反過擬合關鍵變數第一次變得可見；MVP 零 schema 變更。
- **Why now**: 純前端、與 REF 系列無檔案衝突（新 section 檔）；落庫版可等 FEAT-002 與 schema 決策。
- **MVP scope**: 每次 run 追加 `{ time, strategyHashSync, 參數摘要字串, netReturn, sharpe, maxDD, tradeCount }` 進 session 陣列；「本次階段嘗試 N 次」計數；點列還原策略（沿用 F7 之前的最簡形式：只還原，無對比）。
- **Phase plan**: P1 session 內 → P2 落庫（0002 migration，需核准）→ P3 嘗試次數納入 F2 旗標（data-mining 警示）。
- **Validate first**: 無（MVP）；P2 前需 schema 決策。
- **Not yet**: 不做搜尋/篩選/圖表化。
- **Suggested backlog tasks**: `FEAT-006 session run log`（S-M）。

## 6. Bad Ideas / Tempting Traps

| Idea | 為什麼誘人 | 為什麼是陷阱 | 何時可能值得 |
| --- | --- | --- | --- |
| **實盤/自動下單整合** | 「回測完直接跑真的」是所有人的幻想終點 | 直接毀掉「驗證工作站」的定位與安全邊界（金鑰、風控、法遵全部進場）；paper→promoted lifecycle 都還沒做 | Phase D 之後、且 paper-live 流程跑穩一年 |
| **Alerts / Webhooks / 即時行情串流** | 看起來只是「加個通知」 | 引入常駐連線、背景任務、通知可靠性整條新工程線；與 CSP/離線優先架構衝突 | Phase B discovery 穩定後，作為獨立模組評估 |
| **AI 全自動閉環（生成→驗證→迭代無人值守）** | 這是專案的終極願景，STRATEGY_DISCOVERY 都寫了 | 它被明文放在 Phase D 是有原因的：validator、Gate、Test 揭露紀律任一未成熟，閉環就是過擬合永動機 | Phase C 人工審批流程跑順之後 |
| **多資產組合回測** | 「真的交易是組合層面的」——正確 | 引擎、資料模型、指標全要改版；單資產信任鏈都未閉環 | 單資產 Gate/Score 上線後 |
| **雲端同步 / 協作 / 帳號系統** | 「換台電腦繼續」很合理 | 與 local-first 承諾正面衝突；金鑰與資料隱私面爆炸；單人使用者 | 出現真實的多裝置/多人需求時 |
| **插件/自訂指標市集** | 延展性幻覺 | 任意程式碼執行 = 打穿全部安全邊界；白名單 DSL 就是刻意的反插件設計 | 大概永遠不（DSL 擴充是正道） |
| **手機版 / 響應式重製** | PWA 出身的執念 | 桌面研究工作站的核心互動（熱力圖、replay、多面板）在手機上無意義 | 只有「查看報告」需求時做唯讀報告頁（F11 的延伸） |
| **現在就做 walk-forward optimization 全自動版**（F10 advanced） | 反過擬合的正統終點 | 與 Phase B 的 split/embargo/job runner 設計完全重疊，現在做＝拋棄式原型；重運算也不該在前端跑 | Phase B discovery job runner 落地時一併設計 |
| **Dark mode / 主題系統** | 便宜的視覺升級 | inline style 遍地的現況下要先做樣式系統重構；REF 系列剛把樣式搬進 panelStyles，再疊主題會把 move-only 重構變成 rewrite | REF 系列完成且樣式集中後，作為 S 級任務 |
| **社群策略排行榜** | 增長幻想 | 公開績效排行 = cherry-picking 競賽，與信任校準使命相反 | 不做（與使命衝突） |
