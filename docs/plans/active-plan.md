# AlphaFactorForge：持續 AI 研究、跨市場驗證與模擬交易整合計畫

> 實作紀錄（2026-09-21）：P00–P08 已完成；P09 程式／本機驗證已完成而認證來源驗收仍待 token；本輪依 §5 一次一個 phase 完成 **P10 FinMind／TWSE 台灣 ETF adapter**。
> P10 已以五檔真實公開來源做有界 smoke，FinMind raw 與 TWSE 月行情逐列一致；0050 分割／停牌及 00713 配息案例通過。範例成本未確認，五檔 snapshot 均誠實維持 degraded。
> 使用契約見 [`../market-source-tiingo-v1.md`](../market-source-tiingo-v1.md)，交接見
> [`../../handoffs/2026-09-21-p09-tiingo-v1.md`](../../handoffs/2026-09-21-p09-tiingo-v1.md)。
> 純計算契約與限制見 [`../etf-semantics-v1.md`](../etf-semantics-v1.md)，驗證／交接見
> [`../../handoffs/2026-09-20-p08-etf-semantics-v1.md`](../../handoffs/2026-09-20-p08-etf-semantics-v1.md)。
> 下文保留原始規劃基準；P11 及後續功能未在本輪啟動，整體最終 Acceptance Criteria 尚未全部完成。

## 1. 目標、基準與交付限制

### 產品目標

以 **AlphaFactorForge 為唯一主要開發專案**，吸收 AlphaBTC 的資料稽核、研究紀律、模擬帳務、恢復與操作經驗。

完成後，使用者能：

1. 在 VS Code 透過 Codex／ChatGPT 訂閱連接研究工具。
2. 設定研究市場、目標及預算，持續產生不同策略並回測。
3. 保存成功、失敗、拒絕及中斷紀錄，依據開發資料改善探索方向。
4. 區分歷史表現良好、通過獨立驗證與實際 forward 模擬表現。
5. 對合格策略開立虛擬帳戶，以新到達的真實行情持續執行。
6. 從清楚的介面辨識目前工作、資料來源、歷史結果、帳戶與停止原因。

**成功標準是建立可信、可持續運行的研究流程；不以一定找到獲利策略作為工程驗收條件。** 找不到合格策略時，必須如實呈現原因。

### 已確認的產品決策

| 項目 | 決策 |
|---|---|
| 主專案 | AlphaFactorForge；AlphaBTC 保留參考及歷史資料 |
| 第一個 AI 提供者 | Codex／ChatGPT 訂閱 |
| 操作入口 | Tauri 桌面介面與 VS Code 共用同一服務 |
| 背景運行 | 目前 Windows 電腦；關閉 VS Code、桌面介面後仍可執行 |
| 首批市場 | 主流加密貨幣、美股 ETF、台股 ETF |
| 資料預算 | 免費優先；ETF 先做日線 |
| 研究目標 | 收益、低回撤、平衡三種研究設定，分開比較 |
| 模擬開戶 | 預設人工確認；可對指定計畫預先授權自動開戶 |
| 自動化邊界 | AI 產生經驗證的 JSON DSL；不得自行改寫產品程式 |
| 實盤 | 不納入 |

### 規劃基準

| 專案 | 本機分支／HEAD | 唯一任務入口 |
|---|---|---|
| AlphaFactorForge | `docs/alphabtc-capability-transfer`／`05a4d2d885f772db3183d37c30474e57330c41e2` | 根目錄 `tasks.md` |
| AlphaBTC | `docs/autonomous-research-assessment`／`58b1683b1a4837abcd5e3cb836f83f18dd024414` | 根目錄 `task.md` |

兩個工作區於規劃時檢查皆乾淨。AlphaFactorForge 本機分支比本機記錄的 `origin/main` 領先兩個文件提交；規劃及本次文件落盤未確認遠端最新狀態，未 fetch、push 或建立 PR。

另一位 agent 的 `handoffs/2026-09-15-alphabtc-capability-transfer-v1.md` 為既有輸入，保留其 ownership 修正及 C01–C20、ABC 工作對照。不得重新導入已被修正的雙宿主 recovery 問題。

規劃階段受 Plan Mode 限制，原先僅在對話交付。**本文件依使用者後續明確授權，完整保存已確認計畫；本次唯一檔案變更為 `docs/plans/active-plan.md`，不開始 implementation。** 文件位置：

`C:\Users\memor\OneDrive\桌面\AlphaFactorForge\docs\plans\active-plan.md`

規劃階段僅完成唯讀分析、官方文件查核與少量公開資料端點探查；未實作、未運行 AI 研究、未修改研究資料，也未重跑工程測試。本次文件落盤不改變上述功能或驗證狀態。

---

## 2. 現況審視與承接原則

### AlphaFactorForge 已完成的能力

應直接延伸，不重新建立：

- React／Tauri 桌面介面、圖表、手動策略、回測、參數掃描及策略庫。
- 純 TypeScript 計算核心與 Rust parity 核心。
- 版本化策略／資料雜湊及不可變資料匯入。
- Train／Validation／Test 切分與 embargo。
- 多種基準、Random Entry、Gate、Score。
- 不可變 validation records。
- 背景 discovery runner、固定工作池、原子候選提交。
- 暫停、恢復、取消、孤兒工作恢復及 desktop single-instance。
- 事件解析、每個狀態區塊獨立排序，以及結果與輸入綁定。

### 必須補足的缺口

| 領域 | 現況與影響 |
|---|---|
| AI | AI／秘密儲存命令仍有 `NotImplemented`；尚未完成訂閱接入 |
| 策略生成 | discovery 主要是有限參數搜尋；既有 DSL schema 不等於可執行的完整 DSL |
| 研究記憶 | 缺少跨輪假說、血緣、失敗分類、探索預算與可見資料範圍 |
| 歷史保存 | 部分 summaries／trades 仍是最新結果覆寫；不能當成完整歷史實驗帳本 |
| 服務 | 工作生命週期仍與桌面宿主相連；缺獨立服務與跨宿主 ownership |
| ETF | 日線年化目前使用 365；缺交易日、配息、分割及股票市場成交假設 |
| 驗證 | 尚未完成跨輪試驗管理、一次性 Test 消耗與還原防重設 |
| 模擬帳戶 | 缺持久帳務、真實行情排程、恢復及策略版本鎖定 |
| UI | 目前操作訊息仍位於頁首；尚缺統一工作中心、歷史／本次結果辨識 |
| 文件 | `AGENTS.md` 部分 schema-only 描述落後實作；不得據此忽略已完成 runner |

### 從 AlphaBTC 吸收的內容

採取「契約、反例、測試資料先移植，程式按架構重寫」：

- 原始回應與修訂稽核、資料可用時間、不可變快照。
- 假說登記及完整拒絕紀錄。
- 因果成交、成本、帳務對帳、共用 backtest／paper 執行規則。
- 持久風控停止、帳戶隔離、checkpoint 與重送防重。
- 引擎封存、完整工作區備份、隔離還原。
- 工作來源、重連、接續上次帳戶及固定通知。

不直接搬入 AlphaBTC 正式 DB、帳戶或鎖定狀態；不照抄其固定候選、資料長度門檻、Bootstrap 次數及所有權實作。

AlphaBTC 已發現的「累積 trial 增加，但 Bootstrap 精度不足」必須成為新系統的回歸案例。

---

## 3. 目標架構與資料契約

### 3.1 單一服務與計算核心

```text
Tauri 桌面 ─ typed tauri-client ─ Tauri bridge ─┐
                                             ├─ Application Runtime
VS Code Codex ─ MCP stdio adapter ────────────┘        │
                                                      ├─ 持久排程／研究協調
                                                      ├─ Codex app-server adapter
                                                      ├─ 市場資料 adapters
                                                      ├─ 既有 Rust discovery runner
                                                      ├─ 共用成交／帳務核心
                                                      └─ SQLite＋不可變 artifacts
```

- 保留 `src/core` 純 TypeScript、`discovery_core` 純 Rust 的邊界。
- application runtime、DB、網路、排程與程序管理位於純核心之外。
- 在既有 Cargo package 增加 service binary，與桌面共用 orchestration 模組；不建立第二套引擎。
- 先移出 runner 的 Tauri event sink，以及 DB 初始化對 `AppHandle` 的依賴。
- 桌面沒有既存 owner 時，保留嵌入模式；已有服務時，桌面只做代理。
- 啟用背景模式時，先停止接受新工作、完成 checkpoint、釋放 ownership，再啟動服務；不得雙寫。

### 3.2 所有權、控制介面與恢復

採用 Windows 本機 loopback HTTP 控制介面：

- 僅綁定 `127.0.0.1`，使用動態 port。
- endpoint manifest 放在工作區的本機應用資料目錄。
- 使用隨機控制 token、限目前使用者的檔案權限、Host／Origin 檢查。
- token 由 Tauri backend／MCP adapter 使用，不傳給前端。
- 不提供任意 shell、SQL、檔案路徑或實盤端點。

所有權採用：

1. OS 排他檔案鎖，先於 migration 取得。
2. SQLite ownership epoch，所有工作提交檢查世代。
3. 每 5 秒 heartbeat；30 秒未更新標示服務失聯。
4. heartbeat 過期不能單獨授權搶占仍持有 OS 鎖的程序。
5. SQLite busy timeout 預設 5 秒；migration／recovery 僅 owner 可執行。

保留 desktop single-instance。休眠、時鐘跳動、舊 worker 回報均不得導致雙寫。

新增版本化命令 envelope：

```text
protocolVersion、workspaceId、requestId、command、payload
```

- `requestId` 保證重試不重複建立工作。
- 事件包含持久 `eventId`、ownership epoch、entity ID、事件版本。
- DB commit 後發布事件；重連先讀 snapshot，再續接 cursor。
- 保留既有 discovery feed 每個狀態區塊獨立排序的行為。

### 3.3 儲存與相容性

保留現有 DB 位置及舊資料識別，不因改版搬移使用者 DB。新 artifacts 存於其相鄰、非 OneDrive 的本機資料目錄。

| 新增資料群組 | 必要內容 |
|---|---|
| Runtime | ownership、冪等請求、持久事件 |
| Market | instrument、venue、幣別、session calendar、交易規格、來源能力 |
| Provenance | 原始回應 hash、請求範圍、取得時間、修訂關係、品質事件 |
| Snapshot | 原始價／調整方式、calendar／corporate-action／cost profile 版本 |
| Campaign | 市場、目標、資料切分、預算、排程、恢復及自動開戶政策 |
| Hypothesis | 機制、適用條件、失效情境、父策略、變異方式、DSL hash |
| Research attempt | 每次提交、拒絕、執行、完整結果、交易、引擎及輸入指紋 |
| AI invocation | 模型、prompt hash、允許上下文、原始輸出、驗證結果、用量及中斷狀態 |
| Paper | 帳戶、凍結策略、訂單、成交、帳務事件、checkpoint、風控鎖 |
| Evidence registry | 試驗家族、驗證揭露、Test 預約／消耗、還原辨識 |

規則：

- 新研究結果以 attempt ID 保存完整不可變 artifacts；舊 summary 保留為相容的最新結果投影。
- 不宣稱能恢復過去已被覆寫的交易明細。
- 既有 hash／contract 版本不改寫；新市場語意使用新版本識別。
- 舊資料標為 legacy；未知來源、未知調整方式不能自動取得新資格。
- artifacts 先 staging、校驗、原子更名，再提交 DB 參照；未被引用的檔案可辨識，已引用檔案不得悄悄刪除。
- migration 採新增序號，保留 0001–0003 原文；升級前一致備份，失敗回滾，舊 binary 遇較新 schema 必須拒絕寫入。
- 秘密不進 SQLite、artifacts、前端或 logs。

---

## 4. 研究、資料、AI 與模擬行為

### 4.1 市場與資料來源

第一版採現貨／非槓桿 ETF、單商品策略、做多／持有現金；不加入融資、放空、期貨、選擇權或跨幣別組合。

預設研究清單：

- Crypto：BTCUSDT、ETHUSDT，1h。
- US ETF：SPY、QQQ、VTI、TLT、GLD，1d。
- TW ETF：0050、006208、0056、00878、00713，1d。

這是可編輯的研究範圍，不是投資推薦。每次 campaign 凍結清單及商品上市期間，不以目前存在的 ETF 代表全市場歷史。

| 市場 | 主要來源 | 補充與限制 |
|---|---|---|
| Crypto | Binance 公開月／日封存＋同交易所 REST | 校驗 CHECKSUM、時間單位與修訂；Coinbase 作獨立來源，不混填 |
| US ETF | Tiingo EOD 免費帳戶 | 保存 raw、adjusted、dividend、split 欄位；開通時驗證實際權限 |
| TW ETF | FinMind `TaiwanStockPrice` 原始日線 | TWSE 月行情核對；不依賴付費還原價接口 |
| Calendar／事件 | TWSE、NYSE／相應交易所、ETF 發行商 | 收錄休市、臨時停市、分割、除息及付款日期 |

Binance 封存存在時間單位變更及後續修訂，匯入不能只檢查 OHLC。參考[官方封存說明](https://github.com/binance/binance-public-data)。

Tiingo 提供 EOD 與公司事件欄位，但免費方案、使用限制及實際資料權限須在設定時核對；不假設免費帳戶有即時行情。參考 [EOD 文件](https://www.tiingo.com/documentation/end-of-day)、[方案說明](https://www.tiingo.com/about/pricing)。

FinMind 原始價與還原價的權限不同；ETF 事件涵蓋不能僅從接口名稱推定。參考[技術面](https://finmind.github.io/tutor/TaiwanMarket/Technical/)、[基本面](https://finmind.github.io/tutor/TaiwanMarket/Fundamental/)。

#### 補資料流程

1. 依 instrument、session calendar、上市／停牌期間產生預期資料範圍。
2. 比對缺漏、重複、未收盤、無效值及來源修訂。
3. 同來源有界重試、分段下載及快取。
4. 第二來源產生獨立比對證據，不直接覆蓋。
5. 衝突保存兩份原件，待裁定後產生新 snapshot。
6. 缺漏未解決時，顯示阻擋範圍及可執行的補查動作。

不得 forward-fill 成交量、刪除不利期間或將不同 venue／quote 拼成同一條交易資料。

規劃時已讀到 TWSE 的公開規格與 0050 月行情，但**尚未證明上述整組商品的長期資料完整性**。

#### ETF 必要語意

- 交易日與時區來自版本化 calendar；不得把週末或停牌當資料缺漏。
- 保留原始成交價；調整價只按明確用途使用。
- 訊號所用調整不得引入未來尚未發生的公司事件。
- 配息除息日登記應收款，付款日轉可用現金；缺付款日不得假設可立即再投資。
- 未確認公司事件完整性時，結果標示 `DEGRADED`，阻擋合格晉級。
- 分割同步調整股數、成本及未成交訂單；不得產生虛假收益。
- CAGR 按實際經過時間計算；波動年化使用市場／頻率設定，舊版 365 行為保留於舊契約。
- USD、TWD、USDT 帳戶分開，不自動換匯或合併資產收益。
- 使用者確認交易成本設定後才允許資格評估；報表揭露佣金、滑價、交易稅及未納入的個人所得稅。

Calendar 與 ETF 配息來源參考 [NYSE 交易時間](https://www.nyse.com/trade/hours-calendars)、[TWSE ETF 資訊](https://www.twse.com.tw/zh/products/securities/etf/products/div.html)。

### 4.2 訂閱 AI 接入

使用兩條入口：

- **VS Code → MCP → 研究服務**：設定計畫、提交策略、查詢開發結果、暫停與恢復。
- **研究服務 → Codex app-server → JSON 提案**：不依賴 VS Code 視窗持續開啟。

MCP 是操作介面；持續排程由 AlphaFactorForge 負責。官方依據：[MCP](https://developers.openai.com/codex/mcp/)、[app-server](https://learn.chatgpt.com/docs/app-server)。

實作要求：

- 使用官方 ChatGPT 登入流程，由 Codex 管理登入憑證；不把訂閱轉成 API key。
- 本機已見 `codex-cli 0.140.0`；先做版本與能力檢查，不假設最新文件接口全部可用。
- 以 stdio 啟動 app-server，處理初始化、模型列表、turn、結構化輸出、取消、登入及額度狀態。
- 使用專用生成環境，只提供允許的開發摘要。
- 禁用 shell、外部工具、網路搜尋、子 agent、全域記憶及無關 MCP／plugins；不能驗證隔離時，禁止 unattended 模式。
- 不複製使用者認證檔、不自動改寫全域 Codex 設定。
- 模型由設定畫面從實際可用列表選擇，campaign 凍結實際模型 ID。
- AI 只提案，不能批准自己、執行 SQL、取得保留資料或修改閘門。
- 原始行情是否能送給第三方須依來源授權決定；預設只傳必要摘要。
- 配額耗盡進入 `WaitingQuota`；登入失效進入 `AuthRequired`。
- 不自動購買額度、不切換付費 API、不輪替帳號繞過限制。
- 外部成功但本機未記錄的中斷標為 `UnknownOutcome`，計入預算，不盲目重送。

### 4.3 策略 DSL 與經驗累積

先使用兩個核心都已支援的指標交集。新增能力須逐項完成 TS／Rust parity 後才能加入 AI 白名單。

DSL 驗證涵蓋：

- 操作子參數數量、型別、有限值、lookback、參數引用及因果時序。
- 深度最多 8、節點最多 64；保留現有較嚴格限制。
- 不含任意程式碼、檔案操作、網路、動態 import。
- 凍結成本與風控設定，不把它們當作 AI 搜尋軸。
- AI proposal、hypothesis、DSL、候選入隊以可追溯方式連結。

經驗庫採 SQLite 結構化查詢與全文索引，不先引入向量資料庫。

保存：

- 假說與失效條件。
- 父子策略與變異類型。
- 有效／重複／無效／無交易／成本侵蝕／不穩定等結果分類。
- 訓練期間表現、參數敏感度及 regime 摘要。
- 嘗試次數、模型、資料與引擎版本。

預設探索比例為 60% 新機制／結構、40% 既有機制深化。此比例是可凍結設定，不以 Validation／Test 表現自動調整。

預設單次生成最多 4 個提案、單日最多 16 次呼叫、64 個候選；單次模型呼叫逾時 5 分鐘。所有上限都計入失敗及格式錯誤，不能無限自動修復。

### 4.4 防止研究記憶污染驗證

保留既有 60／20／20 加 embargo 作為預設外層切分：

- Train：允許 AI 學習、內層 walk-forward、搜尋及參數診斷。
- Validation：批次候選凍結後才評估，用於確認及預先定義的排名。
- Test：候選與決策凍結後，一次性揭露。

Validation／Test 數值、圖形、失敗原因及據此產生的候選選擇不得進入下一輪 AI 開發上下文。

持續探索可以繼續使用開發區間；已揭露期間不能重新冒充未見資料。需要新獨立證據時，等待後續資料或使用真正未揭露區間。

新增 evidence registry，按標準化商品、期間及揭露範圍追蹤，不能只按 dataset hash 判定。換來源、改名稱、重匯入不能重設 Test。

- Test 執行前先持久預約消耗；執行失敗仍保留消耗。
- 同時選定的多個候選以一個凍結批次消耗。
- registry 位於工作區之外的本機應用資料目錄；還原時採消耗紀錄聯集。
- registry 缺失、回退或來源不明時停止資格流程。
- 不宣稱能防止使用者刻意刪除整台電腦全部紀錄；這種情況不得保留原有資格主張。

既有資料若無法證明未被手動 holdout／圖表揭露，標示 `legacy_exposure_unknown`。

### 4.5 驗證與多目標排名

保留原 `gate-v1`、`score-v1` 舊語意，不降低既有驗收門檻。

新 autonomous protocol 額外要求：

- 成本後報酬、樣本長度、交易數、walk-forward、成本壓力及參數穩定性。
- 三個目標均先經共用硬閘門；合格後分別以淨年化收益、較小回撤、Calmar 排序。
- 只比較相同市場、期間、資料與成本假設的結果。
- 無交易、樣本不足、非有限指標不得用替代數值取得資格。

統計確認使用版本化的 block-bootstrap／多重比較協定：

- 比較淨收益對零，以及相同持有期間的基準差額；區分「可能正收益」與「可能超越基準」。
- block 長度在讀取確認結果前凍結。
- 每個確認批次登記全部有效研究嘗試；被篩掉的候選不能從試驗數消失。
- 批次內 Holm 校正；跨確認批次採預先登記的 alpha spending。
- 執行前計算抽樣精度是否足夠；Bootstrap 上限不足時回報 `NOT_ELIGIBLE`。
- 不直接複製 AlphaBTC 固定 1,000 次抽樣。
- 對純噪音資料做偽陽性模擬，保存檢定實作與隨機種子。

產品狀態分開保存：

- 工作狀態：排隊、運行、暫停、完成、失敗。
- 證據狀態：`PASS`、`REJECT`、`DEGRADED`、`NOT_TESTED`、`NOT_ELIGIBLE`。
- 策略階段：探索、確認、Test 已消耗、paper 運行、停止。

工作完成不等於策略合格。

### 4.6 Forward paper

- 每個帳戶凍結 strategy、DSL、資料來源、成本、風控與引擎 artifact。
- 新策略版本另開帳戶；既有帳戶不被 AI 改寫。
- 預設 USD／USDT 10,000、TWD 1,000,000 虛擬本金；可於開戶前修改。
- 預設一個計畫最多 3 個活動帳戶、工作區最多 10 個；超限排隊。
- 自動開戶須有已保存的計畫授權；AI／MCP 不能自行擴大授權。

共用成交與帳務核心處理：

- 因果訊號、延後成交、費用、滑價、交易單位及現金不足。
- 訂單／成交冪等、持倉與現金對帳。
- 公司事件、帳戶 checkpoint、風控鎖定。
- 資料過期、錯誤行情或版本不符時禁止新增曝險。
- 風控觸發後不自動解除；減倉仍須有合格可成交資料。

ETF 免費日線模式明確標為 **日線延遲確認模擬**：

- 收到已完成行情後決策，最早只能對之後的交易時點下單。
- 訂單必須在目標開盤前存在，才能事後用該開盤價確認模擬成交。
- 分別保存經濟成交時間與資料觀測／確認時間。
- 服務離線期間不補造本來沒有提交的訂單。
- 不以 EOD 結果冒充即時券商成交。

研究排程與 paper 排程獨立：AI 配額耗盡不能停止既有 paper 帳戶。

### 4.7 UI 流程

保留現有圖表與進階工作區，增加以下主入口：

| 頁面 | 使用者要理解的問題 |
|---|---|
| 總覽 | 現在在做什麼、為何等待、下一步是什麼 |
| 資料中心 | 來源、最新時間、涵蓋範圍、缺漏及研究資格 |
| 研究計畫 | 市場、目標、預算、探索方向、進度及停止原因 |
| 策略與經驗 | 做過什麼、為何失敗、哪些策略互為版本 |
| 驗證結果 | 開發／確認／Test 證據與限制 |
| 模擬帳戶 | 固定策略、最新行情、持倉、績效、帳務及風控 |
| 設定與診斷 | Codex、資料來源、背景服務、備份與版本 |

必要互動：

- 首頁提供「接續上次工作」，先列既有帳戶，再提供建立入口。
- 顯示「歷史保存結果」「本次工作」「尚未執行的草稿」。
- 每個結果附工作 ID、產生時間、snapshot／strategy／engine 指紋。
- 新背景結果只通知，不自動替換正在查看的結果。
- 固定通知中心可在深捲動時操作，錯誤保留至確認。
- 關閉通知不取消工作；切換頁面不取消工作。
- 失效選取、連線失敗、資料不存在及版本不符分開呈現。
- 偏好只保存 ID；帳戶與工作以服務 DB 為準。
- 第一次設定採「來源 → 資料預檢 → 研究目標 → 預算 → 啟動」流程。

---

## 5. 實作位置與有界執行階段

### 主要檔案／模組

以下以 AlphaFactorForge 根目錄為基準。既有路徑已查證。

| 既有位置 | 修改方向 |
|---|---|
| `alpha-factor-forge/src-tauri/src/main.rs` | 嵌入／連接模式、owner 與桌面 bridge |
| `alpha-factor-forge/src-tauri/src/lib.rs` | 保持純核心邊界 |
| `alpha-factor-forge/src-tauri/src/discovery_runner/` | 解耦 Tauri sink，接入持久研究協調 |
| `alpha-factor-forge/src-tauri/src/db/` | 路徑注入、migration、新紀錄與原子提交 |
| `alpha-factor-forge/src-tauri/src/commands/ai_commands.rs` | 接入 provider application service |
| `alpha-factor-forge/src-tauri/src/commands/secret_commands.rs` | OS 安全儲存，僅提供狀態 |
| `alpha-factor-forge/src/core/strategy-dsl/` | 型別驗證及純 DSL 執行 |
| `alpha-factor-forge/src/core/backtest/`、`src/core/metrics/` | 版本化市場語意與共用帳務 |
| `alpha-factor-forge/src/services/` | 保留既有 context、split、gate、score、record，新增研究契約 |
| `alpha-factor-forge/src/tauri-client/` | typed API、事件與重連 |
| `alpha-factor-forge/src/components/BacktestPanel.tsx`、`DiscoveryPanel.tsx` | 接入工作中心及結果接續 |

**Planned new files／modules：**

- `alpha-factor-forge/src-tauri/src/service_main.rs`
- `alpha-factor-forge/src-tauri/src/runtime/`
- `alpha-factor-forge/src-tauri/src/providers/`
- `alpha-factor-forge/src-tauri/src/research/`
- `alpha-factor-forge/src-tauri/src/paper/`
- `alpha-factor-forge/src-tauri/src/mcp/`
- `alpha-factor-forge/src/components/ResultsExplorer.tsx`
- `alpha-factor-forge/src/components/ActivityCenter.tsx`
- 各階段新增 migration、對應 TS／Rust 契約測試與 fixtures。

不刪除 AlphaBTC、不重寫既有 0001–0003、不導入新前端框架。網路、OS lock、keychain、時區等必要依賴在所屬階段集中加入並鎖版，禁止無關升級。

### 階段執行規則

**每次 Implementation Agent 只執行一個 phase，完成驗證、文件與交接後停止。**

未指定 phase 時，選第一個未完成且依賴滿足的 phase。後續階段必須有新的明確授權。每個 phase 都必須保持既有功能可建置、適用測試通過；未完成功能保持禁用，不留下會破壞 runtime 的半成品。

下表每列皆為獨立 review 邊界；其範圍不包含後續列的功能。

| Phase | 工作與主要模組 | 依賴 | 驗收／停止點 |
|---|---|---|---|
| P00 契約與相容性預檢 | runtime／AI／市場契約；核對本機 Codex 能力及必要依賴，登記現況與產品決策 | 無 | 列出可用／不可用能力與阻擋原因；不啟動研究、不導入正式資料 |
| P01 Results Explorer | 既有 records、summaries、trades 查詢與歷史結果 UI | P00 | 可重開查看已保存結果；缺歷史明細如實呈現；不改計算 |
| P02 Runtime 解耦 | runner event sink、DB path initialization、共用 orchestration | P00 | 既有桌面行為、golden、runner 原子性不變；未加入 headless 排程 |
| P03 跨宿主 ownership | OS lock、epoch、冪等請求、事件帳本、migration 保護 | P02 | 雙啟、崩潰、休眠、舊 worker 提交測試通過 |
| P04 Headless 與 bridge | service binary、loopback API、Tauri 連接模式 | P03 | 關 UI 工作持續；重連採用同一 run；嵌入模式 native smoke 保留 |
| P05 完整研究歷史 | hypothesis、attempt、lineage、不可變結果 artifacts | P03 | 重跑不覆寫舊明細；失敗可追溯；新舊投影一致 |
| P06 資料基礎契約 | instrument、calendar、raw、revision、snapshot | P03 | 時間單位、缺漏、重複、來源混淆及修訂 fixtures 通過 |
| P07 Crypto adapter | Binance 封存／REST、同來源補查、品質報告 | P06 | BTC／ETH 實際涵蓋報告；已知缺漏仍拒絕；未宣稱多年完整 |
| P08 ETF 市場語意 | calendar、年化、split、dividend、native currency、成本契約 | P06 | 配息／分割／休市／付款日反例及 TS／Rust parity 通過 |
| P09 US ETF adapter | Tiingo、來源設定、公司事件原件匯入 | P08 | 預設 ETF 可逐一報告資格；免費權限或付款日不足有明確阻擋 |
| P10 TW ETF adapter | FinMind、TWSE 核對、ETF 事件與交易日 | P08 | 民國日期、成交量單位、0050 分割／停牌、配息案例通過 |
| P11 可執行 DSL | 既有 DSL validator、TS evaluator、Rust evaluator | P05 | 白名單策略 exact／容差 parity；非法輸出不能進 runner |
| P12 研究可行性與試驗帳本 | campaign、內層 walk-forward、統計精度預檢 | P05、P06、P11 | 不足樣本與 AlphaBTC 精度反例被阻擋；試驗計數不可重設 |
| P13 一次性驗證 | Validation／Test reveal registry、凍結批次及選擇 | P12 | 併發、重試、崩潰、重匯入、重疊期間均不能二次取得新資格 |
| P14 引擎封存與備份 | binary／build manifest、完整工作區 backup／restore | P03、P05、P13 | 校驗、隔離還原、消耗聯集、缺原引擎拒絕；未做 paper |
| P15 Codex 訂閱 adapter | app-server、OS credential 邊界、受限生成環境 | P00、P11、P12 | 一次有界真實生成、配額／登入／逾時測試；無自動研究循環 |
| P16 VS Code MCP | stdio MCP、研究工具白名單、typed service client | P04、P05、P12 | VS Code 可提交／查詢／暫停同一工作；不能揭露 Test 或改閘門 |
| P17 持久研究排程 | campaign scheduler、training-only memory、廣度／深度探索 | P07、P12、P13、P15 | 有界多輪運行、去重、預算停止、重啟續跑；ETF 啟用另需 P09／P10 |
| P18 成交／帳務共用核心 | backtest／paper event kernel、AlphaBTC 差異 fixtures | P08、P11 | 因果、費用、現金／持倉、公司事件、重送對帳；舊 golden 不被覆寫 |
| P19 單帳戶 paper | 帳戶版本鎖、訂單、ledger、checkpoint、kill latch | P14、P18 | 一個受控行情序列完整恢復與對帳；尚不宣稱長期實測 |
| P20 真實 paper 排程 | 新行情觀測、EOD 語意、多帳戶隔離、自動開戶授權 | P17、P19、相應 adapter | 無回補假成交；AI 停止不影響帳戶；資格與數量限制有效 |
| P21 整合 UX | onboarding、工作中心、資料來源、接續、固定通知 | P01、P04、P17、P20 | 真實桌面驗收切頁、深捲動、重啟、失效 ID、延遲回應、不混帳 |
| P22 Windows 運行驗收 | 同使用者登入排程、服務健康、更新、磁碟及長時間運行 | P14、P20、P21 | 七天 soak＋ETF 至少五個交易日，故障演練與交接完成 |

P00 若發現本機 Codex 版本不能滿足安全隔離或必要協定，將 **AI unattended 功能**標為阻擋；其他不依賴 AI 的階段仍可規劃，但不得暗中改成付費 API 或 GUI 點擊自動化。

---

## 6. 驗證、完成條件與交接

### 分層驗證

| 層級 | 必須確認 |
|---|---|
| 靜態／建置 | TypeScript typecheck、Rust check、前後端 build |
| 自動測試 | Vitest、Rust、既有 golden／parity、DB 原子性與 migration |
| 整合 | 真 DB、service／desktop／MCP、provider fixtures、restart |
| 真實外部接線 | 小範圍來源下載、一次 Codex 生成、實際新行情 |
| UI／視覺 | Playwright mock 回歸＋真實 Tauri 操作證據 |
| 長時間運行 | 連續紀錄、故障恢復、資源上限、日線事件時間 |

適用命令沿用：

- `npm.cmd run typecheck`
- `npm.cmd test`
- `npm.cmd run build`
- `npm.cmd run e2e`
- `cargo check --locked`
- `cargo test --locked`

純文件 phase 不啟動研究或完整測試。瀏覽器 mock E2E 不代替原生 Tauri bridge 驗證；native startup smoke 不代替完整命令／事件回路。

### 必測情境

- 首次啟動、第二宿主、owner crash、休眠喚醒、時鐘變動。
- 重複命令、事件漏失、亂序、提交前後中斷。
- 新資料修訂、缺漏、非交易日、停牌、未完成 K 線。
- 股息應收／付款、分割、現金不足及交易單位。
- DSL 非法、零交易、非有限結果、策略去重。
- AI 格式錯誤、登入失效、配額耗盡及未知呼叫結果。
- Test 併發消耗、重疊期間、更換來源、還原舊備份。
- 舊引擎缺失、artifact 被修改、舊版本讀取新 schema。
- 多帳戶隔離、重送成交、持久風控鎖、更新後版本不符。
- 舊結果與新工作並存、切頁、重啟、深捲動及晚到回應。
- 純噪音資料的錯誤晉級率與跨輪選擇偏誤。

所有破壞性故障測試使用隔離工作區，不操作使用者正式研究／帳戶。

### 最終 Acceptance Criteria

1. 關閉 VS Code 與桌面後，授權中的研究服務仍可執行；重新開啟可接續原工作。
2. Crypto、US ETF、TW ETF 均完成至少一條真實來源的下載、稽核、回測及結果保存流程。
3. 每個候選在回測前已保存假說、DSL、來源及版本；失敗與拒絕可查詢。
4. 多輪探索能使用允許的訓練經驗，且自動測試證明 Validation／Test 不進提案上下文。
5. 重跑、換資料 ID、重啟及備份還原不能重設 Test 消耗。
6. 配息、分割、交易日、成本與原幣帳務有可重現驗證。
7. 合格策略能開啟固定版本的虛擬帳戶，用新到達的真實資料持續執行。
8. AI 配額不足、缺資料、引擎缺失及風控停止都有可理解、可追查的狀態。
9. 使用者能辨識所看的是哪次工作、哪份資料、歷史結果或本次結果；深捲動仍看得到操作回饋。
10. 七天 soak、ETF 五個交易日及必要故障演練有實際證據；不據此宣稱策略已獲利或達生產級可靠性。
11. 沒有合格策略時，系統仍正確完成研究並清楚報告，不降低門檻來製造成功。

### 文件與交接

本次規劃不修改其他文件。後續每個實作 phase 才按實際成果更新：

- 唯一任務入口 `tasks.md`，對照既有 Phase B／C、ABC 與本計畫 phase。
- `STRATEGY_DISCOVERY.md`、相關契約及既有 roadmap。
- README 的已交付／未交付狀態與 Windows 啟動說明。
- 既有 handoff 規範；歷史 handoff 僅追加 Resolution，不改寫原紀錄。
- 實際驗證證據、資料來源限制、版本影響與已知問題。

每次交接至少記錄：phase、實際範圍、修改檔案、migration／版本影響、測試與原生驗收結果、隔離資料位置、未完成項目、blocker、Git／PR 狀態及下一階段前置條件。

GitHub 暫時不可用不阻擋本機分析；不得因此假設遠端已同步。恢復發布時重新核對基底與差異，依既有流程建立獨立分支／PR，不推 main、不自行 merge。

**每個 phase 完成交接即停止，不自行啟動下一階段。**
