# AlphaFactorForge (formerly QuantDesk) - Project History

> 給接手 agent / 開發者的完整上下文。
> 撰寫日期：2026-06-27
> 匯入註記：本檔由使用者提供的歷史附件匯入到目前工作區根目錄。內容保留當時敘述；若與目前檔案位置或驗證狀態不同，請以 `README.md`、`tasks.md`、`AGENTS.md` 和實際程式碼為準。

---

## 一、專案緣起與目標

使用者要求從零開始建立一個**區塊鏈交易策略 PWA**，具備：
1. 抓取歷史行情資料（多交易所）
2. 即時行情（WebSocket + polling 容錯）
3. 自訂買賣策略並做**回測**
4. **模擬交易**（Paper Trading，用即時資料）

定位：**進階交易者自用工具**，資訊密度高，風格近終端機美學（淺色）。

---

## 二、目前部署形態

**現在是 PWA（Pure Frontend）**，所有邏輯跑在瀏覽器：
- 主檔：`QuantDesk.dc.html`（專案根目錄）
- 行情直接從 Binance / OKX / Coinbase 公開 API 抓，不走 proxy
- 策略回測在前端計算（Canvas Worker）
- 資料凍結存在 localStorage / 下載為 JSON

**架構已預留未來多部署形態的路**（見下節）。

---

## 三、架構演進決策記錄

### 決策 1 — 放棄純 PWA，定位為 Tauri Desktop App（已規劃，未實作）
- 理由：AI API key 不能放前端、大量 Discovery 不能在 UI 主執行緒跑、需要本機 SQLite 持久化
- 決策：**Frontend 保留現有 Web UI（React/Canvas）**，加上 Tauri shell + Rust backend
- 目前狀態：架構設計完成，Phase A scaffold 已產出（`quantdesk/` 子目錄），**尚未本機驗證**

### 決策 2 — Worker 只做前端輔助
- UI Worker（`src/workers/backtest.worker.ts`）只跑單次回測/短 sweep/圖表計算
- 大量 Strategy Discovery **一律走 Tauri Rust backend**（thread pool + checkpoint + event 協定）

### 決策 3 — AI 生成策略走 DSL，不走 code string
- AI 只產 JSON 運算樹（白名單指標+運算子）
- frontend validator（`src/core/strategy-dsl/validator.ts`）+ Rust 端鏡像 validator 雙層驗證
- code mode 保留給人工使用，AI 永不走 code mode

### 決策 4 — Train/Validation/Test 三層隔離
- Train：AI 只看 Train 摘要
- Validation：策略排名依據
- Test：神聖，每策略只揭露一次，燒毀後不再參與調整
- 詳見 `STRATEGY_DISCOVERY.md`

---

## 四、現有 PWA（QuantDesk.dc.html）功能清單

### 圖表引擎
- K 線 Canvas 繪製（自製，不依賴 lightweight-charts）
- 滾輪縮放（以游標為錨點）、拖曳平移、雙擊重置
- 滑鼠懸停顯示 OHLCV tooltip + 十字準星
- 指標圖層：MA(可調) / EMA / BB(布林通道) / RSI(獨立面板) / 成交量
- 買賣標記（▲▼）疊加在 K 線上

### 資料層
- 多交易所容錯：Binance → OKX → Coinbase（自動或手動選）
- 分頁深抓歷史（近 500 / 2000 / 5000 / 最大可得）
- 週期：1m / 3m / 5m / 15m / 1h / 4h / 1d
- 資料凍結（快照）：↓ JSON / ↓ CSV 匯出、↑ 載入再用（可重現回測）
- 狀態列顯示實際來源（「歷史 Binance」、「即時 OKX」等）

### 即時模擬（Paper Trading）
- WebSocket（Binance）/ polling（OKX/Coinbase）即時更新
- 系統時間顯示（防「是否當機」疑慮）
- 模擬買/賣/清倉手動按鈕
- 即時損益、持倉量顯示

### 策略編輯器
三種模式（可切換）：
1. **參數模式**：MA 快慢線、RSI、MACD、ATR、Stochastic 等 preset 指標，只調數值
2. **規則積木**：進場/出場條件分別用下拉積木組合（指標 + 運算子 + 值），AND 全成立
3. **程式碼模式**：直接寫 JS 運算式（`price`, `rsi`, `macd`, `bb.upper`, `atr`, `stoch.k`... 全部可用）
- 每個模式有專屬「?」說明按鈕（點了跳到對應說明分頁）
- 詳細變數手冊：`STRATEGY_GUIDE.md`
- 內建 5 個預設策略（EMA 金叉 / RSI 反轉 / MACD / BB 突破 / 組合策略）

### 回測引擎
- 手續費 / 滑點 / 部位大小 % / 成交價（收盤/次開）/ 多空方向 可設定
- 交易所建議費率快選（Binance VIP0 / OKX Standard / Coinbase）
- Bar Magnifier 盤中精算（開啟後每根 K 線下抓更細子 K 線，精準判定 SL/TP 先後順序）
- 回測時間區段：真實日期選擇（非只能靠根數），區間外 K 線灰化顯示
- 「回測中」黃色過渡動畫按鈕
- 資料來源顯示在回測區間區塊（確保再現性）

### 回測結果面板
**一般模式**（圖表下方）：
- 19 項績效統計（淨報酬、夏普、索提諾、卡瑪、最大回撤、獲利因子...）
- 小型權益曲線 sparkline（含 B&H 比較線）

**專注數據模式**（收合圖表，點「▢ 專注數據」）：
- 大字大圖版績效面板
- 完整權益曲線 + 水下回撤圖
- Round-trip 逐筆交易表（含 MAE/MFE 欄位）
- 樣本內/樣本外對比（Holdout 功能）
- 報告匯出：↓ 績效報告 JSON / ↓ 交易明細 CSV

### 參數掃描（Heatmap）
- 點「掃描」開啟彈窗
- 選 X/Y 軸參數 + 範圍，產出 N×M 熱圖
- 顏色編碼（淨報酬 / 夏普 / 勝率）
- 點格子可套用到目前策略
- 含說明面板

### 策略庫
- 本地保存策略（localStorage），支援命名與切換
- 「儲存策略」「另存新策略」

### Bar Replay（逐 K 線重播）
- 回測模式下開啟：圖表逐根前進，重播歷史
- 播放/暫停/步進/速度控制

---

## 五、設計規範

- **字體**：IBM Plex Mono（monospace）/ 系統 sans-serif
- **色系**：淺色終端機——背景 `#f8f6f1`，邊框 `#e6e2d9`，文字 `#16150f`，上漲 `#11875a`，下跌 `#b23b2e`，強調 `#d6862a`
- **密度**：高資訊密度，無 emoji，無圓角卡片風格
- **圖表**：純 Canvas（自製，非第三方圖表庫）

---

## 六、Tauri Desktop Scaffold（已產出，未本機驗證）

位置：`quantdesk/`（子目錄，可獨立下載）

### 已建檔案
**Rust Backend**（`src-tauri/`）
- `Cargo.toml`：tauri v2, rusqlite（bundled）, serde, sha2, hex
- `tauri.conf.json`：window 設定、allowlist
- `main.rs`：invoke handler 全部註冊
- `error.rs`：統一錯誤型別
- `db/mod.rs`：連線池 + migration runner
- `db/repositories.rs`：datasets / candles / strategy CRUD
- `commands/db_commands.rs`：多數完整，save/get_backtest_result 待補
- `commands/file_commands.rs`：export_report stub
- `commands/ai_commands.rs`：Phase C stub
- `commands/secret_commands.rs`：Phase C stub（keychain）
- `commands/discovery_commands.rs`：Phase B stub
- `migrations/0001_init.sql`：9 張表（schema 已立）

**前端 Core 純函數**（`src/core/`，✅ 完整，可直接 `npm test`）
- `indicators/`：SMA/EMA/WMA/RSI/MACD/ATR/BBANDS/STDDEV/HIGHEST/LOWEST/ROC
- `metrics/`：完整回測統計
- `backtest/`：deterministic 回測引擎
- `hashing/`：strategy_hash / dataset_hash
- `strategy-dsl/schema.ts`：白名單型別定義
- `strategy-dsl/validator.ts`：whitelist 編譯器/驗證器

**Frontend Bridge**（`src/tauri-client/`）
- `commands.ts`：所有 invoke 的 typed wrapper
- `events.ts`：discovery 事件訂閱 + throttle
- `dbClient.ts`：importDataset（含 hash）

**設定 / Shell**
- `package.json`、`tsconfig.json`、`vite.config.ts`、`index.html`
- `src/main.tsx`：bridge 驗證殼（非完整 UI）
- `src/workers/backtest.worker.ts`：Worker 協定骨架

**文件**
- `README.md`：完整說明
- `TODO.md`：逐檔狀態 + 可能爆的 compile error + Phase B/C 規劃
- `PHASE_A_VERIFY.md`：本機驗證 checklist + compile error 修法 + command/DB/bridge 對應表

### 本機驗證指令
```bash
cd quantdesk
npm install
npm test           # 純邏輯測試（應全綠，不需 Rust）
npm run typecheck  # TS 型別檢查

cd src-tauri
cargo check        # 需本機 Rust 1.77+
cargo tauri icon path/to/logo.png   # 產生圖示（必做）
cargo tauri dev    # 啟動桌面 app
```

---

## 七、待完成事項

### PWA 端（QuantDesk.dc.html）
- RSI 指標面板有時不反應（已知 bug，需追查 drawRsi 的 candles ref）
- MA 可調參數（已加在指標面板，但 drawChart 的 maLen 需再確認對應）
- Bar Replay 完整整合（基礎架構已建，細節待完善）
- PWA manifest / Service Worker（`manifest.webmanifest` 已存在，sw 尚未建立）

### Tauri Desktop（quantdesk/）
詳見 `quantdesk/TODO.md`。核心 Phase B 待做：
- Train/Validation/Test split + embargo + walk-forward
- Gate + Score + Benchmark
- Discovery job runner（Rust thread pool）
- Results Explorer UI
- lifecycle 管理（candidate/validated/rejected）

Phase C（AI Lab）：
- keychain 整合（`keyring` crate）
- generate_strategy_dsl + 後端 validator 鏡像
- 人工 approve 流程

---

## 八、關鍵設計文件

| 文件 | 說明 |
|------|------|
| `STRATEGY_GUIDE.md` | 策略編輯器使用手冊（三種模式的變數清單、語法、範例） |
| `STRATEGY_DISCOVERY.md` | Strategy Discovery Engine 設計文件 v3（Tauri 定案版） |
| `tasks.md` | 目前唯一 active task board；已合併原 `task.md` 的 PWA feature map |
| `quantdesk/TODO.md` | Tauri scaffold 逐檔狀態 + compile error 修法 |
| `quantdesk/PHASE_A_VERIFY.md` | Phase A 本機驗證 checklist + command/DB 對應表 |

---

## 九、不可違反的邊界

1. **API key 不進 frontend / localStorage / SQLite 明文**（只走 keychain）
2. **frontend 不直接呼叫 AI API**（所有 AI 呼叫走 Tauri backend command）
3. **AI 只能產 JSON DSL**；validator 擋未知運算子與注入字串
4. **大量 Discovery 不跑在 UI 主執行緒**（走 Tauri backend job runner）
5. **Test segment 不參與 v1 ranking**；Results Explorer 預設隱藏 Test
6. **code mode = manual-only**，AI 不可使用
