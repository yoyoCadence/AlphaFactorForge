# 策略自動探索引擎 (Strategy Discovery Engine) — 設計文件 v3

> 機構級防過擬合版本。核心原則：**搜尋空間自動化，但驗證紀律不可妥協。**
>
> **部署形態（v3 定案）：Tauri Desktop App。** Frontend 保留現有 Web UI / React / Canvas；AI API 呼叫、API key 管理、本機資料庫（SQLite）、長時間探索任務（job runner）全部由 **Tauri backend** 負責。不再以純前端 PWA 或 serverless proxy 為架構目標。

## 目標
開啟獨立分頁後，自動化地長期（數週至數月）：
1. **生成策略** — 傳統指標窮舉 + AI 生成 DSL 指標
2. **嚴格驗證** — Train / Validation / Test 三層隔離，杜絕資料窺探
3. **漸進精細化** — 粗→族群分類→方向細化→微調
4. **生命週期管控** — candidate → validated → paper_live → promoted，未經實戰不得標記可用
5. **三層持久化** — 定義 / 摘要 / 明細分離，供事後過擬合檢查

---

## 0. 不可妥協的紀律（Design Invariants）

這些是整個引擎的地基，任何功能都不得違反：

1. **Test 集神聖不可侵犯** — Test 區段的任何統計，永不參與生成、排名、微調、AI 提示。只在策略「即將晉級」時揭露一次，揭露後該策略的 Test 結果即「燒毀」（標記 used，不可重複用於決策）。
2. **AI 全程看不到 Validation / Test** — AI 只接收 Train 區段的摘要特徵。
3. **沒有任意程式碼執行** — AI 只能輸出受限 JSON DSL，由白名單編譯器解析。前端、Worker、Tauri backend 都不存在 `eval` / `Function` / 動態 import / 動態指令執行路徑。
4. **API Key 永不進前端、永不明文落地** — 由 Tauri backend 透過 OS keychain / secure storage 管理；前端、localStorage、SQLite 一律不得存放金鑰明文。
5. **必須贏過 benchmark** — 跑不贏 Buy&Hold / SMA / RSI / Bollinger / Random，一律淘汰。
6. **前後端職責分離** — 前端只負責 UI 與輕量互動運算；所有重任務、持久化、AI 呼叫由 backend 負責（見第 4 節）。

---

## 1. 資料切分：Train / Validation / Test （需求 1）

### 切分方式
對每個（幣種 × 時間框）的歷史序列，依時間順序切三段（**時間序，不可隨機洗牌**，避免未來資訊洩漏）：

```
|<--------- Train 60% --------->|<-- Validation 20% -->|<-- Test 20% -->|
  AI 可見、參數搜尋               策略排名、晉級篩選        最終評分、僅一次
```

### 使用規則
| 區段 | 誰能讀 | 用途 | 禁止 |
|---|---|---|---|
| **Train** | 生成器 + AI | 參數搜尋、AI 提示特徵 | — |
| **Validation** | 排名器 | 策略排名、Gate 篩選、族群分類、微調方向 | AI 不可見 |
| **Test** | 晉級裁決器 | 最終分數，決定 promoted | 不得參與任何調參；每策略僅用一次 |

### Embargo / Purge
Train 與 Validation 交界處留 **embargo gap**（例：指標最長回看期 + 持倉期），切斷指標回看造成的洩漏。Validation/Test 交界同理。

### Walk-Forward（進階）
除單次切分外，提供 **滾動 walk-forward**：多個 (Train→Val) 視窗向前滾動，最後統一在最末段 Test 驗證。降低「剛好挑到好時段」的運氣成分。

---

## 2. 指標庫與 Strategy DSL （需求 2）

### 白名單指標（只有這些能被引用）
```
趨勢： EMA, SMA, WMA, MACD, ADX
動量： RSI, STOCH, CCI, ROC, MOM
波動： ATR, BBANDS (Bollinger), STDDEV, KELTNER
量能： OBV, VOL_SMA, MFI
價格： CLOSE, OPEN, HIGH, LOW, HLC3, HIGHEST, LOWEST
```

### 白名單運算子
```
算術： ADD, SUB, MUL, DIV, ABS, MIN, MAX, CLAMP
比較： GT, LT, GTE, LTE, CROSS_UP, CROSS_DOWN
邏輯： AND, OR, NOT
時序： SHIFT(n), RISING(n), FALLING(n)
常數： CONST(x)
```

### JSON DSL 範例
AI 與生成器都只輸出這種結構：
```json
{
  "name": "EMA pull-back + RSI filter",
  "params": { "emaFast": 12, "emaSlow": 50, "rsiLen": 14, "rsiBuy": 40 },
  "entry": {
    "op": "AND",
    "args": [
      { "op": "CROSS_UP", "args": [
        { "ind": "EMA", "src": "CLOSE", "len": "$emaFast" },
        { "ind": "EMA", "src": "CLOSE", "len": "$emaSlow" } ] },
      { "op": "LT", "args": [
        { "ind": "RSI", "len": "$rsiLen" },
        { "op": "CONST", "v": "$rsiBuy" } ] }
    ]
  },
  "exit": {
    "op": "CROSS_DOWN",
    "args": [
      { "ind": "EMA", "src": "CLOSE", "len": "$emaFast" },
      { "ind": "EMA", "src": "CLOSE", "len": "$emaSlow" } ]
  }
}
```

### 編譯器（DSL → 可執行）
- 遞迴解析 AST，每個節點 `op`/`ind` 必須在白名單，否則整個策略**拒絕**。
- 參數 `$xxx` 只能引用自身 `params`，範圍與型別受 schema 限制（例：`len` 為 2..400 整數）。
- **絕對禁止**：`import` / `eval` / `exec` / `fetch` / `fs` / `network` / `file` / `while` / 自訂函數字串。DSL 沒有迴圈、沒有 IO、沒有字串求值——它只是一棵運算樹。
- AST 深度上限（例：8 層）與節點數上限（例：64），防組合爆炸與惡意巨型樹。
- 編譯失敗 → 丟棄、記錄原因、AI 重試下一個。

---

## 3. AI 生成模組（受控） （需求 2 + 3）

### 工作流
```
每 batch（例：每 25 個策略驗證完）：
  1. 從「Validation 排名前段」反推（注意：給 AI 的特徵只來自 Train 表現）
     提供：策略 DSL 結構 + Train 區段摘要統計（CAGR、交易數、PF）
  2. Tauri backend 呼叫 AI，要求：輸出 N 個「新的 DSL JSON」（嚴格 schema，附 few-shot 範例）
  3. backend 用白名單編譯器驗證每個 DSL
  4. 通過者進入候選隊列；失敗者丟棄
  5. 標記來源 = ai，記錄 prompt 指紋供日後審計
```
> 註：首版（見第 12 節）AI 為「最小 Strategy Lab」——人工 approve 才入隊，不做全自動閉環。

### API Key 安全架構（Tauri，需求 3 定案）
```
[ Frontend (React) ]  --invoke('ai_generate', {prompt})-->  [ Tauri backend (Rust) ]
   不接觸 key                                                    |  從 OS keychain 取 key
   只收到結果/錯誤                                                v
                                                          [ Claude API ]
```
- **金鑰存放**：OS keychain / secure storage（macOS Keychain、Windows Credential Manager、Linux Secret Service），由 Tauri backend 存取。
- **絕對禁止**：localStorage、SQLite、前端記憶體、設定檔明文存放金鑰。
- 前端「AI 設定」面板只負責：觸發 `set_api_key`（值直送 backend 寫入 keychain，前端不留存）、按「測試連接」、顯示連線狀態。**前端永遠不讀回 key。**
- Rate limit、retry、配額耗盡降級（自動關 AI 線、只跑傳統窮舉）都在 backend 處理。

### 沙箱保證
即使 AI 被提示注入，它能輸出的最壞情況也只是「一棵不合法的 DSL 樹」→ 編譯器拒絕。沒有任何路徑能讓 AI 文字變成可執行程式碼。

---

## 4. 運算架構：Tauri backend job runner + 輔助 Worker （需求 4 定案）

職責一刀切分：**前端只做 UI 與輕量互動運算；重任務全在 Tauri backend。**

```
┌─ Frontend (React / Canvas) ─┐   ┌─ 輕量 Web Worker ─┐   ┌─ Tauri backend (Rust) job runner ─┐
│ UI、表格、圖表、互動         │   │ 單次互動回測       │   │ 大量 Strategy Discovery            │
│ 派發 job、訂閱 event         │──▶│ 短 sweep           │   │ 長時間任務 / pause / resume        │
│ 每 300ms / 10 筆 batch 更新  │   │ 圖表指標預計算     │   │ checkpoint / 斷點續跑              │
│ 不做重運算、不寫 DB          │◀──│ (不碰 DOM/state)   │   │ SQLite 讀寫                        │
└──────────────────────────────┘   └────────────────────┘   │ AI API 呼叫（持金鑰）              │
            ▲                                                 └──────────────┬─────────────────────┘
            └────────────── event protocol (jobId + progress/result) ───────┘
```

### Web Worker（輕量輔助，保留）
- 定位：**只做前端輕量運算**——單次互動回測、短 sweep、圖表指標預計算。
- **限制**：不得操作 DOM / React state / Canvas DOM；不接收 function callback。
- 通訊：以 **jobId + postMessage event protocol**（`{type, jobId, payload}`），不傳函數。

### Tauri backend job runner（重任務）
- 承擔：大量 Strategy Discovery、長時間任務、pause / resume / checkpoint、SQLite 寫入、AI API 呼叫。
- **Job 協定**：前端 `invoke('discovery_start', cfg)` 取得 `jobId`；backend 以 Tauri event（`discovery://progress`、`discovery://result`、`discovery://done`）回報；前端訂閱後節流更新（每 300ms 或每 10 筆）。
- **pause / resume**：`invoke('discovery_pause'|'resume'|'cancel', {jobId})`；狀態與進度寫入 SQLite checkpoint，App 重啟亦可續跑。
- **平行度**：backend 依 CPU 核心數開 worker thread pool 跑回測，與前端完全解耦。
- **K 線快取**：同（幣種×時間框×區段）只讀一次，backend 內共享，後續策略複用。

---

## 5. 評分系統：Gate + Score （需求 5）

評分分兩關。**先過 Gate（硬門檻，淘汰制），才計 Score（排名用）。**

### 5.1 Gate（硬性條件，任一不過即淘汰）
| 條件 | 預設門檻 | 理由 |
|---|---|---|
| 最少交易數 | ≥ 30（依區段長度調整） | 樣本太少無統計意義 |
| 成本後平均每筆收益 | > 0 | 扣手續費滑點後仍須為正 |
| Rolling window 正報酬比例 | ≥ 55% 的滾動視窗為正 | 不能靠單一爆發 |
| Max Drawdown | ≤ 設定上限（例 35%） | 風險控制 |
| 單月貢獻上限 | 任一月貢獻 ≤ 總獲利 40% | 防單一行情運氣 |
| 單筆貢獻上限 | 任一筆 ≤ 總獲利 25% | 防一筆暴利掩蓋爛策略 |
| 優於 benchmark | 見第 6 節 | 必須有超額價值 |

### 5.2 Score（通過 Gate 者排名，全部用 OOS = Validation/Test）
```
Score =  w1·OOS_CAGR
       + w2·Sortino
       + w3·Calmar
       + w4·RegimeRobustness     // 跨牛/熊/盤整一致性
       + w5·ProfitFactor
       + w6·Consistency          // 月報酬標準差的倒數
       − p1·ComplexityPenalty    // DSL 節點數 / 參數量，越複雜扣越多
       − p2·TurnoverPenalty      // 換手率過高（交易成本脆弱）扣分
       − p3·DataMiningPenalty    // 已測組合數越多，門檻越嚴（多重比較校正）
```

- **ComplexityPenalty**：抑制「為了擬合而堆指標」。簡單且有效者勝出。
- **TurnoverPenalty**：高換手對成本假設極敏感，懲罰之。
- **DataMiningPenalty (Deflated Sharpe 精神)**：測試越多策略，最佳者「純靠運氣」的機率越高。隨累計測試數動態抬高顯著性門檻。
- 各權重 `w*`、懲罰 `p*` 可在 UI 調整並存檔，但 **Test 分數只在晉級裁決時計算一次**。

---

## 6. Benchmark 比較 （需求 6）

每個策略在**相同（幣種×時間框×區段）**下，必須贏過下列基準才能晉級：

| Benchmark | 說明 |
|---|---|
| **Buy & Hold** | 期初買進持有到底 |
| **SMA cross** | 標準 50/200（或時間框對應）均線 |
| **RSI 30/70** | 教科書超買超賣 |
| **Bollinger mean reversion** | 觸下軌買、觸上軌賣 |
| **Random Entry（相同持倉期）** | 蒙地卡羅：隨機進場、持倉期與該策略相同，跑 N 次取分布 |

- **Random Entry 是關鍵**：策略必須顯著優於「相同曝險、相同持倉時間的亂買」（例：超過隨機分布的 95 百分位），否則它的報酬只是 beta / 持倉時間造成，不是真 alpha。
- 未同時贏過全部基準 → Gate 失敗，不進排名。

---

## 7. 微調：策略族群分類 （需求 7）

不再用「前 5 名參數是否重疊」這種粗略判準。改為**策略 clustering**：

### 族群定義（依行為特徵分類，非依指標名稱）
| 族群 | 行為特徵 |
|---|---|
| **趨勢追蹤 (Trend)** | 順勢、低換手、贏在大波段、勝率可低但賺賠比高 |
| **均值回歸 (Mean-Reversion)** | 逆勢、高勝率、小賺多次、怕趨勢盤 |
| **爆量突破 (Breakout)** | 量價齊揚進場、抓啟動、假突破多 |
| **其他 / 混合** | 不明確歸類者 |

### 分類特徵向量
用回測行為而非指標名稱分類，避免「不同指標其實同行為」：平均持倉期、勝率、賺賠比、與大盤相關性、進場時的波動/量能分位、換手率。對前段策略做 k-means / 階層式分群。

### 進入方向細化的條件
```
IF 某族群在 Validation 前 10 名中占比 > 60%:
    → 進入第二階段，只在該族群方向細化（變異該族群的參數與結構）
ELSE:
    → 市場呈多模態，繼續第一階段寬搜，暫不收斂
```

避免過早收斂到單一方向，也避免把不同行為的策略硬湊在一起細化。

---

## 8. Meme / 低品質幣種風險過濾 （需求 8）

對 meme 與小幣，標準 OHLCV 回測會嚴重高估（滑價、無法成交、插針）。加入風險特徵與過濾：

### 風險特徵（每幣種 × 時間框計算）
| 特徵 | 定義 | 用途 |
|---|---|---|
| **Volume Explosion Ratio** | 當期量 / 近 N 期均量 | 偵測 pump |
| **Liquidity Decay** | 近期均量相對歷史高峰的衰減 | 偵測退潮、出貨後乾枯 |
| **Pump Exhaustion** | 急漲後動能/量能背離 | 偵測派發末端 |
| **Wick Risk** | 上影線 / 實體比、插針頻率 | 偵測流動性陷阱 |
| **Spread Proxy** | 用高低價差 / 收盤估計點差 | 成本真實性 |

### 過濾與降權規則
- **自動跳過**：上市時間太短（K 線根數不足）、均量低於門檻、估計點差過大、極端上影線頻率過高。
- **降權**：通過但偏弱者，在 Score 上乘風險折扣（liquidity-adjusted）。
- **成本加成**：低流動性幣的回測自動套用更高滑價假設（動態滑價 = f(Spread Proxy, Volume)）。
- meme 幣的策略即使分數高，晉級時 Gate 更嚴（需更長存活期、更多 paper trading）。

---

## 9. 三層持久化 — SQLite（需求 9 定案）

不再只存綜合評分。分三層存於 **SQLite**（Tauri backend 管理），方便事後過擬合 / 穩健性鑑識與 SQL 查詢：

```
Table 1 — strategy_def（小、永久）
  { id, dsl_json, param_schema, source: traditional|ai, ai_prompt_hash?,
    strategy_hash, created_at, lifecycle }

Table 2 — backtest_summary（中、每個策略×資料組合一筆）
  { strategy_id, dataset_hash, symbol, interval, segment: train|val|test,
    cagr, sortino, calmar, max_dd, profit_factor, trades, turnover,
    gate_passed, score, bench_bh, bench_sma, bench_rsi, bench_boll, bench_random_pctile }

Table 3 — trade_detail / equity_curve（大、可選 / 可外存）
  { strategy_id, dataset_key, trades_blob, equity_blob, drawdown_blob }
```

- **去重雙鍵**：`strategy_hash = hash(正規化 DSL + execModel)`、`dataset_hash = hash(幣種+時間框+區段邊界+資料版本)`。`(strategy_hash, dataset_hash)` 已存即跳過（duplicate skip），載入既有結果，**永不重測相同組合**。
- **資料源定案**：SQLite 為唯一主資料庫（**不用 IndexedDB**）。`localStorage` 只存非敏感 UI preference（theme、last selected tab 等）。**API key 不進 localStorage / SQLite 明文**，走 OS keychain（見第 3 節）。
- **明細保留策略**：Table 3 量大，預設只對 `validated` 以上狀態保留明細；其餘可自動清理或匯出外存。
- **匯出/匯入**：三層打包成檔（SQLite dump 或 JSON）；離線可載回繼續，去重表一併帶走。

---

## 10. 策略生命週期 （需求 10）

每個策略有明確狀態機。**AI 生成且歷史回測通過 ≠ 可用**，必須走完隱藏測試與 paper trading。

```
        生成
         │
         v
    ┌─────────┐  Gate+Score 過 (Validation)   ┌───────────┐
    │candidate│ ────────────────────────────> │ validated │
    └─────────┘                                └─────┬─────┘
         │ Gate 失敗 / 輸 benchmark                   │ 過 hidden Test（僅一次）
         v                                            v
    ┌──────────┐                              ┌────────────┐
    │ rejected │                              │ paper_live │  接即時資料模擬交易
    └──────────┘                              └─────┬──────┘
         ^                                          │ paper 期間維持表現
         │ 任一階段表現崩壞 / 疑似過擬合              v
    ┌─────────────┐  <───────────────────  ┌────────────┐
    │ quarantined │                         │  promoted  │  標記「可套用到實盤」
    └─────────────┘                         └────────────┘
```

| 狀態 | 意義 | 進入條件 |
|---|---|---|
| **candidate** | 剛生成，待驗證 | 生成即是 |
| **validated** | 通過 Gate + Score（Validation 段）+ 贏 benchmark | 自動 |
| **quarantined** | 可疑：跨區段不一致、疑過擬合、paper 衰退 | 自動偵測或人工 |
| **paper_live** | 通過 hidden Test，進入即時 paper trading | Test 僅揭露一次且通過 |
| **promoted** | paper 期間穩定，標記可用 | 達 paper 存活期 + 表現門檻 |
| **rejected** | 淘汰 | Gate 失敗 / 輸 benchmark / paper 崩壞 |

- **hidden Test 一次性**：策略晉級到 paper_live 前，才第一次（也是唯一一次）計算 Test 區段。一旦看過，Test 即「燒毀」，不能再回頭用它調參。
- **paper trading 接即時資料**：用主程式既有的即時模擬引擎，跑真實時間的前進測試（forward test），確認非歷史過擬合。
- 只有 `promoted` 的策略，主頁面「套用」按鈕才允許一鍵載入到實盤前的回測/模擬。

---

## 11. UI 設計（更新）

### 獨立分頁結構
**頂部控制行**
```
[開始探索] [暫停] [恢復] [匯出] [匯入] [AI 設定(keychain)]
幣種多選 | 時間框多選 | 階段: 寬搜/族群細化/微調 | Walk-forward 開關
AI DSL 生成: 開/關（顯示 backend 連線狀態，不顯示 key）
```

**左側 進度面板（340px）**
- 已測 / 去重命中 / 本階段目標
- 目前任務：BTC 1h · Train→Val
- 即時最佳（標 Validation 分數，**不顯示 Test**）
- AI：生成 N、編譯通過 M、被拒 K
- 族群分布長條：Trend xx% / MR xx% / Breakout xx%

**中央 結果表格（Validation 排名）**
| 排名 | 策略 | 來源 | 族群 | 生命週期 | Val CAGR | Sortino | MaxDD | vs Bench | Gate | Score |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | EMA+RSI | AI | Trend | validated | … | … | … | ✓全勝 | ✓ | 0.78 |

- 排序：Score / Sortino / Calmar / MaxDD / 換手 / 來源 / 族群。
- **Test 欄位預設隱藏**，只有策略進入 paper_live 後才顯示其一次性 Test 分數。

**右側 詳細 / 鑑識**
- 選中策略：各區段（Train/Val/Test*）表現對照、權益與回撤曲線、benchmark delta、族群特徵、DSL 樹檢視、（AI）prompt 指紋。
- 生命週期操作：送 paper、quarantine、reject、promote（受條件鎖）。

---

## 12. 實作路線圖（v3 — Tauri 定案）

> 首版交付 = **Phase A + Phase B**，並含 **Phase C 的最小 AI Strategy Lab**。Phase D（paper trading）延後，不納入第一版主要交付。

### Phase A — Tauri 地基（首版）
- [ ] Tauri 專案骨架：Rust backend + 現有 React/Canvas 前端整合
- [ ] SQLite schema（strategy_def / backtest_summary / trade_detail）+ migration
- [ ] 資料匯入：K 線歷史匯入 SQLite（取代 / 補充現有抓取）
- [ ] core backtest / indicator 抽離成**純函數模組**（前端 Worker 與 backend 可共用邏輯規格）
- [ ] 策略保存、回測結果保存（落 SQLite）

### Phase B — 探索骨架與驗證紀律（首版）
- [ ] Train / Validation / Test 時間切分 + embargo
- [ ] DSL schema + 白名單編譯器（AST 驗證、深度/節點上限）
- [ ] Gate 全條件 + Score（含三懲罰項）
- [ ] 5 種 benchmark + Random Entry 蒙地卡羅
- [ ] `strategy_hash` + `dataset_hash` + duplicate skip
- [ ] Discovery **job queue**（Tauri backend job runner）+ pause / resume / checkpoint
- [ ] event protocol（jobId + progress/result/done）+ 前端節流訂閱
- [ ] **Results Explorer**：Validation 排名表、篩選、鑽研、區段對照
- [ ] 生命週期最小集：candidate → validated → rejected

### Phase C — 最小 AI Strategy Lab（首版，僅最小集）
- [ ] API key **secure storage**（OS keychain，backend 管理）
- [ ] AI connection test（前端按鈕 → backend → Claude，回連線狀態）
- [ ] AI 產生 **JSON Strategy DSL**（嚴格 schema + few-shot）
- [ ] **DSL validator**（白名單編譯器把關）
- [ ] **人工 approve** 才入隊（不做全自動閉環）

### Phase D — 延後（不在第一版）
- [ ] paper_live 接即時引擎 forward test
- [ ] hidden Test 一次性揭露的全自動晉級
- [ ] 策略 clustering + 族群占比 >60% 自動細化
- [ ] meme 風險特徵 + 過濾 / 降權 / 動態滑價
- [ ] promote / quarantine 自動偵測、walk-forward 全自動

---

## 13. 決策紀錄（v3 定案）

1. **部署形態 → Tauri Desktop App。** 不以純前端 PWA 或 serverless proxy 為架構目標。前端保留 Web UI / React / Canvas；AI 呼叫、key 管理、SQLite、長任務皆由 Tauri backend 負責。
2. **Worker 定位 → 輕量輔助。** 單次互動回測、短 sweep、圖表指標預計算留前端 Worker；大量 Discovery / 長任務 / pause-resume-checkpoint / SQLite / AI 呼叫交 Tauri backend job runner。Worker 不碰 DOM/state/Canvas，不收 callback，改 jobId + event protocol。
3. **資料源 → SQLite 為主。** 不用 IndexedDB 當主庫；localStorage 只存非敏感 UI preference；API key 走 OS keychain，不明文落地。
4. **首版範圍 → Phase A + B + 最小 AI Lab（Phase C 最小集）。** paper trading（Phase D）延後。

> 設計已定案。下一步進入實作時，從 Phase A 的 Tauri 骨架 + SQLite schema + core backtest 抽離開始。

