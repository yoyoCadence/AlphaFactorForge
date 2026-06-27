# AlphaFactorForge (formerly QuantDesk) - Conversation History

> 撰寫日期：2026-06-27  
> 說明：本文件依時間順序摘錄所有重要對話與決策，供接手 agent / 開發者完整了解背景。
> 匯入註記：本檔由使用者提供的歷史附件匯入到目前工作區根目錄。內容保留當時敘述；若與目前檔案位置、任務板或驗證狀態不同，請以 `README.md`、`tasks.md`、`AGENTS.md` 和實際程式碼為準。

---

## 階段 0 — 初始需求與問答

**使用者初始需求（m0001）**
> 「寫一個區塊鏈可抓歷史資料下來也能抓當下最新價格的PWA，並且會需要能自定義買賣策略做回測與模擬交易」

**Agent 問卷（m0002）**  
Agent 用 questions_v2 工具詢問以下方向：
- 設計風格偏好
- 主要功能範圍
- 目標用戶
- 策略編輯器形式（積木/程式碼/混合）
- 幣種範圍
- 視覺風格偏好

**使用者回答**
- context：完全從零開始，請你決定風格
- scope：兩個主功能 — 新增策略做回測、用當下即時資料做模擬交易
- audience：我自己（進階交易者，資訊密度高）
- strategy_ui：以上都要（積木 + 程式碼 + 參數三種模式）
- assets：BTC/ETH 等主流幣、altcoins、也要股票/其他
- vibe：終端機美學（選項 2）

---

## 階段 1 — 初版建立

**決定設計系統**
- 淺色終端機美學
- 字體：IBM Plex Mono（monospace）
- 色系：背景 `#f8f6f1`、邊框 `#e6e2d9`、文字 `#16150f`、上漲 `#11875a`、下跌 `#b23b2e`

**串接行情 API**
- 主力：Binance 公開 API（歷史 K 線 `/klines` + WebSocket 即時）
- 初期 500 根 K 線
- 支援幣種：BTCUSDT / ETHUSDT / SOLUSDT / BNBUSDT / XRPUSDT / DOGEUSDT / ADAUSDT / AVAXUSDT / LINKUSDT / MATICUSDT

**圖表引擎（純 Canvas 自製）**
- K 線繪製
- 均線（MA）
- RSI 獨立面板
- 成交量
- 買賣標記（▲▼）

**策略編輯器（三種模式）**
1. 參數模式：調數值
2. 規則積木：下拉組合
3. 程式碼模式：JS 運算式

**回測引擎（初版）**
- 長倉 only
- 無手續費/滑點
- 基本績效統計（淨報酬、勝率、最大回撤等）
- 權益曲線 sparkline

**即時模擬（Paper Trading）**
- WebSocket 更新
- 手動買/賣/清倉按鈕
- 損益顯示

主檔建立：`QuantDesk.dc.html`

---

## 階段 2 — 圖表操作、指標擴充、說明按鈕

**使用者需求（m0017）**
> 「圖的便利度要和幣安一樣：可放大縮小、可往左往右拉看更多時間資料、滑鼠上去要能顯示該點價格。要有更多參數供新增規格來支持開發新穎獨創的交易策略。要有?的按鍵可點擊看更多介紹資訊。」

**實作內容**
- 滾輪縮放（以游標位置為錨點）
- 拖曳平移（按住左右拖動看更多歷史）
- 雙擊重置視圖
- 懸停 tooltip（開/高/低/收/量 + 漲跌）+ 十字準星
- 指標擴充：MACD / 布林通道(BB) / ATR / 隨機指標(Stoch K/D) / 量能 MA
- 規則積木的運算元從 8 個擴充到 19 個
- 說明彈窗（主 ? 按鈕）

---

## 階段 3 — 多交易所容錯、資料下載

**問題發現**
沙箱環境把 Binance 三個節點全擋掉（使用者可能也在 Binance 受限地區）。

**實作內容**
- 多交易所容錯層：Binance → OKX → Coinbase，輪流嘗試
- 自動容錯 + 手動選來源（下拉選單）
- 狀態列顯示實際用哪家（「歷史 Binance」、「即時 OKX」）

**資料下載功能（使用者需求）**
> 「要有下載功能，能把資料一次載下來，之後回測才會是用同一份數據，且可回去檢查」

- ↓ JSON 下載當前 K 線資料集（含交易所/幣種/週期/時間戳）
- ↓ CSV 下載
- ↑ 載入 JSON（凍結同一份數據做可重現回測）
- 凍結狀態 banner（頂部黃色提示）
- 凍結後不自動重抓，解凍才恢復即時

---

## 階段 4 — 圖表收合、專注數據模式、task.md

**使用者需求（m0055）**
> 「圖形做成可以縮小，只顯示數據，讓回測時可以更專注於數據本身。請去參考市面回測的軟體，先調查他具體有哪些功能，一一列出，然後寫在 task.md，我們逐步完善」

**市面回測軟體調查**（參考：TradingView Strategy Tester、QuantConnect/LEAN、Backtrader、MetaTrader 5、NinjaTrader）
完整功能清單寫入 `task.md`

**實作：圖表收合 / 專注數據模式**
- 圖表底部「▢ 專注數據」按鈕
- 收合後顯示「回測數據總覽」大字面板
- 19 項績效指標網格（淨報酬、買入持有、夏普、索提諾、卡瑪、最大回撤、獲利因子等）
- Round-trip 逐筆交易表（進場時間/出場時間/持有 K 棒/進出場價/損益%/原因）

**回測指標新增（ext 統計）**
- 平均獲利、平均虧損、盈虧比
- 最大連勝/連敗
- 最大單筆盈虧
- 平均持有 K 棒
- 在市場時間比例

---

## 階段 5 — 回測引擎真實化（Phase 1）

**使用者需求（m0065）**
> 「好 第一階段開始做」（接 task.md 的規劃）

**實作：真實回測引擎**
- **手續費 %（每邊）** — 預設 0.05%
- **滑點 %（每邊）** — 預設 0.02%，買單墊高/賣單壓低
- **部位大小 %** — 用權益的百分比下單
- **成交價**：當根收盤 vs 下一根開盤
- **交易方向**：僅做多 / 僅做空 / 多空反手

**即時模擬也套用費率/滑點**

**交易所建議費率快選（m0140 附近）**
- Binance VIP0：Maker 0.08% / Taker 0.1%，滑點 0.05%
- OKX Standard：Maker 0.08% / Taker 0.1%，滑點 0.06%
- Coinbase Advanced：Maker 0.06% / Taker 0.08%，滑點 0.08%
- 點選後自動填入欄位

---

## 階段 6 — 策略說明、回測時間區段

**使用者需求（m0079）**
> 「回測的策略編輯器是分成三個方式？每個方式應該要有?能看具體說明，尤其程式碼要有一個文件寫相關要求。另外回測應該要能設定時間區段」

**實作**
- 策略說明彈窗（三分頁：參數 / 規則積木 / 程式碼）
- 程式碼模式說明：完整可用變數清單（price/open/high/low/volume/ema/rsi/macd/bb/atr/stoch 等）、撰寫規則、5 組慣用寫法範例
- 建立 `STRATEGY_GUIDE.md`（完整策略開發手冊）

**回測時間區段**
- 初版：% 滑桿（前 X% ~ 後 Y% 的 K 線）
- 圖表顯示區段外灰化

---

## 階段 7 — 真實日期選擇、深度歷史拓取

**使用者需求（m0094）**
> 「區間的選擇不是只能從500根去調整，要真的能調整日期，所以你資料來源也要看，具體能抓到多久以前的，最好全都有」

**實作**
- 回測區間改為真實日期選擇器（date input）
- 快捷按鈕：近 1 週 / 近 1 月 / 近 3 月 / 近 1 年 / 全部
- 資料層改為**分頁深抓**（多交易所回拉）：Binance 每次 1000 根，OKX/Coinbase 各自分段
- 歷史深度選單：近 500 / 2000 / 5000 / 最大可得
- 驗證：5000 根 = 約 7 個月的 1h 資料
- 載入進度顯示（「載入中 3000/5000 根…」）

---

## 階段 8 — 更細週期、Bar Magnifier

**使用者需求（m0117）**
> 「加入 1m / 5m / 15m 更細週期並做 Bar Magnifier 式盤中精算」

**實作**
- 週期列新增：1m / 3m / 5m（原有 15m / 1h / 4h / 1d 不變）
- 三家交易所的 interval 對照表補齊
- **Bar Magnifier 盤中精算**：
  - 回測時在每根 K 線下抓更細的子 K 線
  - 精準判定 SL/TP 哪個先被觸及（解決「同一根 K 線內 SL 和 TP 都被觸及時，哪個先到」的歧義）
  - 驗證：超緊 SL/TP 0.15% 設定下，20 次出場中 2 次因 5m 子 K 線而改變結果，淨報酬從 -3.83% 改善至 -3.25%
  - 超大範圍（> 10000 子 K 線）有防護 cap

---

## 階段 9 — 按鈕狀態、即時系統時間、費率選擇

**使用者需求（m0140）**
> 「目前根本選好參數就自動回測完畢，所以執行回測的按鈕好像沒屁用？...即時模擬的數據真的是即時跑的嗎？我看根本沒變，應該要加系統時間讓使用者看的出來沒有當機」

**實作**
- 「執行回測」按鈕三態：灰色「等待資料」/ 綠色「✓ 執行回測」/ 已回測「✓ 已顯示回測結果」
- 參數更動後自動清除回測結果並恢復綠色按鈕
- 即時模擬頁面加入「當前時間」顯示（每秒更新）

**回測中黃色過渡動畫（m0151）**
> 「雖然回測計算很快，但我覺得要做過渡畫面，回測中（黃色）之類的，這樣肉眼也會看到按鈕狀態有變」

- 點「執行回測」立刻變黃色「⟳ 回測中…」（脈動動畫，不可點）
- 約 400ms 後（計算完成）恢復「✓ 已顯示回測結果」

---

## 階段 10 — 資料來源顯示強化

**使用者需求（m0156）**
> 「目前資料是用哪家要顯示出來，還要能選擇，這樣回測才知道是用同一個資料來源，才有再現性」

**實作**
- `auto` 模式現在顯示「自動容錯（目前 Binance）」（不再只顯示「自動容錯」）
- 資料來源顯示在**回測區間區塊**（strategy 編輯器內，方便確認再現性）
- 來源也帶進「回測數據總覽」的 focusMeta 與下載 JSON

---

## 階段 11 — task.md 剩餘功能完成

**使用者需求（m0161）**
> 「好，繼續完成 task.md 規劃的剩餘階段，能做的全做」

**實作清單**

**(F15) 權益曲線 + B&H 比較 + 水下回撤**
- 大型「回測數據總覽」面板加入完整權益曲線（策略線 vs B&H 線）
- 水下回撤區域（填充色覆蓋最大回撤）
- 小型 sparkline 也加上 B&H 比較線

**(F16) MAE / MFE**
- Round-trip 表格新增 MAE（最大逆向偏移）/ MFE（最大有利偏移）欄位
- focusStats 新增 avgMAE / avgMFE 統計格

**(F17) Holdout（樣本外測試）**
- 回測區間區塊加「樣本外測試」勾選框（預設 20% holdout）
- 勾選後同時跑「樣本內」與「樣本外」兩次回測
- 結果並列：淨報酬 / 夏普 / 勝率 各欄顯示兩組，並標「過擬合警示」

**(F18) 回測報告匯出**
- ↓ 績效報告 JSON（含所有統計 + 策略定義 + 執行模型設定 + 來源）
- ↓ 交易明細 CSV（含 MAE/MFE 欄位）
- 兩顆按鈕在「專注數據」面板右上角

**(F19) 策略庫（本地存檔）**
- 策略編輯器加「策略庫」Bar（展示已儲存策略下拉 + 儲存 + 另存）
- 策略存入 localStorage
- 切換策略時自動載入參數/規則/程式碼

**(F20) 參數掃描（Heatmap）**
- 「掃描」按鈕開啟彈窗
- X/Y 軸選參數，各設 min/max/step
- 產出 N×M 熱圖（顏色 = 淨報酬/夏普/勝率）
- 點格子可套用到目前策略
- 含說明面板；排除零交易格不參與「最佳」計算

**(F21) Bar Replay**
- 回測模式下「▷ Replay」按鈕
- 圖表逐根前進，重播歷史（播放/暫停/步進一根）
- 速度控制（1×/2×/5×）

---

## 階段 12 — 日期顯示、預設策略、說明文件更新

**使用者需求（m0224 附近）**
> - 滑鼠移動到圖表顯示的日期看不出年份
> - 請預設幾個常見策略，做為測試與學習用
> - 新增的功能也要有使用介紹，越詳細越好，同時要更新到 QUANTDESK 使用說明
> - 目前圖表的指標不夠完善（MA 沒辦法調整參數、RSI 沒反應）
> - 回測報告匯出看起來不夠直觀

**實作**

**(1) 日期年份顯示**
- tooltip 日期格式從 `MM/DD HH:mm` 改為 `YYYY/MM/DD HH:mm`（完整含年份）

**(2) 預設策略（5 個）**
1. EMA 金叉：EMA12/50 金叉進場，死叉出場
2. RSI 反轉：RSI < 30 進場，> 70 出場
3. MACD 動能：MACD 從下穿越 Signal 進場，反向出場
4. 布林突破：收盤突破 BB 上軌進場，跌破中軌出場
5. 組合策略：EMA 上升趨勢 + RSI 未超買 + MACD 正向

**(3) 使用說明更新（說明彈窗）**
- 主 ? 按鈕說明彈窗：更新所有新功能（Holdout、MAE/MFE、Heatmap、Replay、Bar Magnifier、凍結資料等）

**(4) 指標完善**
- MA 加入長度參數調整（maLen 可調）
- RSI 不反應問題修復（drawRsi 的 candles 參照修正）

**(5) 匯出按鈕標籤改善**
- 底部工具列「↓ JSON」→「↓ 資料 JSON」（K 線資料集）
- 「↓ CSV」→「↓ 資料 CSV」
- 加上「K 線資料」前綴標籤與詳細 tooltip
- 說明：底部 = 原始 K 線資料集（再現性）；專注數據面板 = 回測績效報告

---

## 階段 13 — 策略模式 ? 說明按鈕

**使用者需求（m0260）**
> 「參數 規則積木 程式碼 都要有? 可看具體說明」

**實作**
- 三個分頁標題旁各加一個「?」圓圈圖示
- 點任一「?」→ 開啟說明彈窗並跳到對應分頁
- 驗證：點程式碼的 ? → 顯示程式碼變數表；點規則積木的 ? → 顯示積木說明

---

## 階段 14 — 參數掃描說明

**使用者需求（m0256）**
> 「參數掃描那邊也要有自己的說明」

**實作**
- 參數掃描彈窗加入說明面板（什麼是參數掃描、使用步驟、注意事項）

---

## 階段 15 — Strategy Discovery Engine 規劃

**使用者需求（m0266）**
> 「請做一個獨立分頁，能夠自動去隨機生成策略，並用不同時間段和幣種做回測，產生報告，只要開啟他就會持續一直去有條理的嘗試各種策略……你需要先思考這個目的要達成所需要的設計，先構思好並記錄下來思路並可跟我討論，沒問題再開始實作」

**Agent 設計構思（m0267）**
設計要點：
- 粗到細的探索策略（先廣搜，再針對有潛力的方向細調）
- 去重機制（避免重測相同組合）
- 可匯出/匯入測試記錄（離線後可繼續，避免重複）

**使用者補充需求（m0268）**
> 「第一階段的指標選擇太少了，要把能用的指標都做進去。這是漫長的馬拉松，可能會跑好幾個月去窮舉所有合理的可能性。也應該要能夠去自動化定義新指標，例如 MA 和 RSI 做加權生成新指標去回測。這塊功能也會需要能夠接 AI API key 去做（或者直接接 CLI）。以上請思考並整合進去計畫的 STRATEGY_DISCOVERY.md」

**撰寫 STRATEGY_DISCOVERY.md v1**
- 粗到細探索框架（Phase 1 傳統策略 186 種 → Phase 2 變形與加權 → Phase 3 AI 生成）
- AI 生成指標（白名單 DSL，後端呼叫，前端 validator）
- 去重（fingerprint hash）
- 匯出/匯入

---

## 階段 16 — 防過擬合升級（m0272）

**使用者需求**
> 「加入 Train / Validation / Test 三層資料切分」「不允許 AI 生成任意 Python/JavaScript 程式碼，改成讓 AI 輸出 JSON DSL」等 10 點要求

**撰寫 STRATEGY_DISCOVERY.md v2（機構級防過擬合版）**
- Train/Validation/Test 三層隔離 + embargo gap + walk-forward
- AI 只能產 JSON 運算樹（white-list 指標+運算子），validator 雙層驗證
- Gate（最低門檻）+ Score（加權評分）+ Benchmark 對比
- 去重（strategy_hash + dataset_hash + segment）
- 指標白名單：20 個指標 + 22 個運算子

---

## 階段 17 — Tauri 架構定案（m0275）

**使用者需求**
> 「本專案直接定位為 Tauri Desktop App，不再以純前端 PWA 為主要目標」

四個決策：
1. 部署形態：Tauri Desktop App（Frontend 保留 Web UI）
2. Worker：只做前端輔助；大量 Discovery 走 Tauri Rust backend
3. AI API key：Tauri keychain，不進 frontend / localStorage
4. 持久化：SQLite（通過 Tauri commands），不用 IndexedDB 作主資料庫

**撰寫 STRATEGY_DISCOVERY.md v3（Tauri 定案版）**

---

## 階段 18 — Phase A Scaffold 生成（m0286 ~ m0302）

**使用者需求**
> 「選路線 1。請依照以下原則執行：目標仍然是 QuantDesk Tauri Desktop Strategy Research Workstation……產出「可交接、可落地、可逐步驗證」的 Tauri Desktop scaffold」

**產出檔案（`quantdesk/` 子目錄）**

Rust Backend：
- `src-tauri/Cargo.toml`：tauri v2, rusqlite（bundled）, serde, sha2, hex
- `src-tauri/tauri.conf.json`
- `src-tauri/build.rs`
- `src-tauri/src/main.rs`（invoke handler 全部註冊）
- `src-tauri/src/error.rs`
- `src-tauri/src/db/mod.rs`（連線 + migration runner）
- `src-tauri/src/db/repositories.rs`（CRUD 完整）
- `src-tauri/src/commands/db_commands.rs`
- `src-tauri/src/commands/file_commands.rs`（stub）
- `src-tauri/src/commands/ai_commands.rs`（Phase C stub）
- `src-tauri/src/commands/secret_commands.rs`（Phase C stub）
- `src-tauri/src/commands/discovery_commands.rs`（Phase B stub）
- `src-tauri/migrations/0001_init.sql`（9 張表：datasets / candles / strategies / backtest_summaries / round_trips / discovery_runs / discovery_run_results / ai_generations / settings）

前端 Core（✅ 完整，可直接 `npm test`）：
- `src/core/indicators/index.ts`：SMA/EMA/WMA/RSI/MACD/ATR/BBANDS/STDDEV/HIGHEST/LOWEST/ROC
- `src/core/metrics/index.ts`：完整統計
- `src/core/backtest/index.ts`：deterministic 回測引擎
- `src/core/hashing/index.ts`：strategy_hash / dataset_hash
- `src/core/strategy-dsl/schema.ts`：白名單型別
- `src/core/strategy-dsl/validator.ts`：whitelist 編譯器/驗證器

Frontend Bridge：
- `src/tauri-client/commands.ts`：typed invoke wrapper
- `src/tauri-client/events.ts`：discovery 事件 + throttle
- `src/tauri-client/dbClient.ts`：importDataset（含 hash）

配置 / Shell：
- `package.json`、`tsconfig.json`、`vite.config.ts`、`index.html`
- `src/main.tsx`（bridge 驗證殼）
- `src/workers/backtest.worker.ts`（Worker 協定骨架）

測試：
- `indicators.test.ts`、`validator.test.ts`、`backtest.test.ts`

文件：
- `README.md`
- `TODO.md`（逐檔狀態 + compile error 修法 + Phase B/C 規劃）
- `PHASE_A_VERIFY.md`（本機驗證 checklist + command/DB 對應表）
- `HISTORY.md`（本文件的前身，高層次摘要）

**發布方式**：`quantdesk/` 打包為 zip 下載

---

## 階段 19 — 文件整理（m0311 ~ m0315）

**使用者需求（m0311）**
> 「目前是PWA而已，但已經做了架構的調整讓未來可以做到不同的部署型態，請你把相關的對話歷史整理到一個 history.md，我要給接手的 agent 了解狀況用的」

→ 建立 `quantdesk/HISTORY.md`（高層次摘要版本）

**使用者需求（m0315）**
> 「另外把關於這個專案的所有對話歷史紀錄都寫到一個 conversation_history.md 裡面」

→ 建立本文件 `quantdesk/CONVERSATION_HISTORY.md`

---

## 附錄 A — 專案檔案清單（截至 2026-06-27）

```
專案根目錄/
├── QuantDesk.dc.html      主 PWA（所有前端功能）
├── manifest.webmanifest   PWA manifest
├── STRATEGY_GUIDE.md      策略開發手冊（變數清單/語法/範例）
├── STRATEGY_DISCOVERY.md  Discovery Engine 設計文件 v3
├── task.md                開發任務清單（完成狀態）
├── support.js             DC runtime（自動產生，勿修改）
└── quantdesk/             Tauri Desktop Scaffold（Phase A）
    ├── README.md
    ├── TODO.md
    ├── HISTORY.md
    ├── CONVERSATION_HISTORY.md   本文件
    ├── PHASE_A_VERIFY.md
    ├── package.json
    ├── tsconfig.json
    ├── vite.config.ts
    ├── index.html
    ├── src/
    │   ├── main.tsx
    │   ├── core/
    │   │   ├── indicators/
    │   │   ├── metrics/
    │   │   ├── backtest/
    │   │   ├── hashing/
    │   │   └── strategy-dsl/
    │   ├── tauri-client/
    │   └── workers/
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json
        ├── build.rs
        ├── migrations/
        └── src/
            ├── main.rs
            ├── error.rs
            ├── db/
            └── commands/
```

---

## 附錄 B — 未解決的已知問題

1. **RSI 偶爾不反應**：`drawRsi` 的 `candles` 參照在某些狀態更新後需重新拉取，確切 repro 步驟：切換幣種後立刻改週期
2. **Bar Replay 細節**：基礎架構已建，Play/Pause/速度控制 UI 已加，但 replay 中「策略訊號對應哪根」需再確認
3. **MA 可調參數**：`maLen` 欄位已加入策略編輯器，但 `drawChart` 中的 maLen 需確認是否正確從 `strat.maLen` 讀取（或仍硬編碼預設值）

---

## 附錄 C — 不可違反的邊界（Tauri 版）

1. API key 不進 frontend / localStorage / SQLite 明文（只走 keychain）
2. frontend 不直接呼叫 AI API
3. AI 只能產 JSON DSL；validator 必須擋下未知運算子與注入字串
4. 大量 Discovery 不跑在 UI 主執行緒（走 Tauri backend job runner）
5. Test segment 不參與 v1 ranking；Results Explorer 預設隱藏 Test
6. code mode = manual-only，AI 不可使用
