# Handoff: P09 驗收審查 — 通過，四項觀察

Date: 2026-09-21
Repo: yoyoCadence/AlphaFactorForge
Reviewed: PR [#111](https://github.com/yoyoCadence/AlphaFactorForge/pull/111)，branch `feat/p09-tiingo-etf-adapter`，
implementation head `8ed8f80`（base main `4e48343`）
Reviewer: Claude Code（獨立驗收，未修改產品程式碼）
Status: **程式與本機驗收通過**。無 P1／P2 defect；四項為觀察事項，不阻擋合併。
審查當時建議**維持 draft、不 merge**；PR #111 其後已由維護者合併為 `396ee56`（見文末 Resolution）。
P09 的真實帳戶外部驗收仍未完成，於 `tasks.md` 維持 In Progress。

## 結論

PR #111 的範圍與 active-plan §5 P09（`Tiingo、來源設定、公司事件原件匯入`）一致，沒有超出：
沒有 migration、沒有新 crate（`windows-sys` 已在 lock 內，只多一條 direct edge）、沒有動到既有
hash／golden／legacy 引擎、沒有新增 Tauri 命令或 UI 路徑、沒有排程器。既有 `fetch`（Binance）
行為未改變，`http.rs` 只多一個 opt-in 的認證建構子。

P09 的驗收／停止點是「**預設 ETF 可逐一報告資格；免費權限或付款日不足有明確阻擋**」。這兩項在
程式、自動測試與原生 CLI 三個層級都有對應證據，且我另以獨立探針複驗（見下）。

文件對「還沒有什麼」的陳述誠實：README、tasks.md、handoff、contract 都明說真實帳戶下載尚未驗收，
`tasks.md` 也把 P09 列為 **In Progress**（而非 Done），未宣稱資料完整性或免費方案權限。

## 我實際驗證了什麼

### 1. 重跑交接文件宣稱的每一項數字

| 項目 | 交接宣稱 | 我實測 |
| --- | --- | --- |
| `npm.cmd test` | 969 | **969 passed**（53 files） |
| `npm.cmd run typecheck` / `build` | pass | pass |
| `cargo test --locked` | 353（71＋280＋2） | **353 passed**（71＋280＋2） |
| 其中 P09 新增 | 16 | **16 passed**（15 × `tiingo_ingest`＋1 × `tiingo_credentials`） |
| `cargo clippy --locked --all-targets` | 只有 5 項既有 | **確認**：`backtest.rs` ×2、`score.rs` ×2、`file_commands.rs` ×1；P09 檔案 0 項 |
| PR CI | — | **六項全綠**（run 35590615895：build／test／typecheck／e2e／cargo-check／native-smoke） |
| `npm.cmd run e2e` | 78/78 | 未於本機重跑（Windows 本機平行執行已知 flake），改以 **CI e2e 綠燈**為證據 |

Rust 建置與測試使用 OneDrive 以外的 `CARGO_TARGET_DIR`，避開既知的連結鎖問題。

### 2. 原生 CLI 實測（隔離工作區，未配置憑證）

`tiingo-status` → `{"configured":false,"reason":"credential_missing"}`，exit 5。
`fetch-tiingo --from 2026-09-01 --to 2026-09-19`（範本設定、隔離 data-dir）→
**五檔各一筆報告，全部 `BLOCKED` / `credential_missing`**，`qualificationEligible` 皆 false，
`datasetId`／`snapshot` 皆 null、`provenanceIds` 空、`bars` 0，exit 5；未發出任何網路請求。
同時確認 exit code 契約：usage `64`、設定／IO／range 失敗 `1`、有阻擋 `5`。
`--token abc123` 被拒（exit 64）且錯誤訊息不含該值；重複旗標同樣被拒。
超過 366 天 → `range_must_be_1_to_366_days`；未完結範圍 → `range_not_finalized`。

### 3. 獨立探針（非重跑既有測試）

以臨時探針（reviewer 自撰，執行後已移除，工作區無殘留）複驗六項吃重不變量，**全部一次通過**：

- **範圍邊界的精確性**：366 天通過、367 天拒絕；完結門檻在 `end+06:00 UTC` **當下通過、早 1 毫秒拒絕**；
  1 天為最小值、`to <= from` 拒絕。（邊界矩陣而非抽樣）
- **403 是逐檔而非整批停止**：SPY metadata 403、QQQ 正常時，SPY `entitlement_denied` 無資格，
  **QQQ 仍獨立完成並取得資格**。這正是 P09 停止點所要求的「逐一報告」，401／429 才整批停止。
- **落盤序列是原始價、snapshot 只能是歷史**：供應商 adjusted（50）與原始價（100）分歧時，
  dataset 存的是 100／1000 與分割後的 50／2000；`priceBasis = Raw`、`kind = Historical`。
- **未知供應商欄位保留在原件**：注入 `someFutureField` 後仍可取得資格，且 artifact 位元組同時含
  `someFutureField` 與 `adjClose`——原件未被解析結果取代。
- **設定嚴格性延伸到巢狀層級**：`instruments[].token` 與 `instruments[].market.token` 都被拒
  （既有測試只涵蓋頂層）。
- **token 不可能流入被拒請求的錯誤字串**：以 token 建立的 fetcher 對非白名單 host／非 HTTPS／
  非預設埠請求，`Display` 與 `Debug` 輸出皆不含 token。

### 4. 安全邊界逐條複核

- `TiingoToken` 無 `Debug`／`Serialize`，只有 `pub(super) header()`；CLI 不接受 token，無環境變數 fallback。
- token 只進 `Authorization` header，不進 URL；`https_only`、`max_redirects(0)`、host 白名單
  僅 `api.tiingo.com`、非 443 埠拒絕。
- `FetchError::Status` 只帶 url 與狀態碼，不帶 body；認證路徑的 transport 訊息被換成固定字串。
- 失敗時只落盤 `{"localFailureReceipt":<code>}`，不假裝取得 response body。
- 設定檔 `deny_unknown_fields` ＋ 256 KiB 上限，先讀設定再開 DB。
- `git diff` 全域掃描無 token／key／password 形態字串；Cargo.lock 僅 +1 行、無新 crate 或升級。

### 5. 範圍與文件一致性

23 個檔案、+1942／-19。新程式碼集中在 4 個新檔＋3 處加法式接線（`mod.rs`、`sources/mod.rs`、
`service.rs`）。`config/tiingo-2026.example.json` 的 2026 NYSE／Nasdaq 日曆我逐項核對：
10 個休市日（含 7/4 週六順延至 7/3）與 11/27、12/24 兩個早收，與官方行事曆一致；
五檔 `corporateActionsConfirmed` 全為 false、`costs` 全為 null——範本預設拿不到資格。
tasks.md／roadmap／README／CHANGELOG／market-contract／active-plan 皆已同步且未誇大。

## 觀察事項（不阻擋合併）

1. **兩處發布紀錄尚未提交**：`handoffs/2026-09-21-p09-tiingo-v1.md` 的 Resolution 段與 `tasks.md`
   的 P09 publication 行只存在於本機 worktree，未推上 PR #111。依 AGENTS.md §6，交接紀錄應入庫。
   本次審查一併提交。
2. **股息金額比對容差極緊**：`|Δ| > 1e-10 × max(amount, 1)` 即阻擋。方向是安全的（寧可阻擋），
   但若 Tiingo 的 EOD 與 distribution 兩個端點對同一筆配息採不同進位，真實帳戶驗收時第一個
   卡住的很可能是 `distribution_amount_mismatch` 而非權限問題。請在實跑時先看這個 reason。
3. **範本日曆僅涵蓋 2026 年**：`calendarFrom 2026-01-01`／`calendarToExclusive 2027-01-01`。
   首次真實驗收的 `--from` 必須落在 2026 年內，否則得到 `calendar_out_of_range` 而非權限資訊。
   要取更長歷史需先補上對應年度的官方日曆證據與新的 calendar 版本。
4. **`retrieve` 每次請求都全量載入該商品的 provenance 列表**再線性尋找前一筆同端點觀測。
   P09 規模（每次最多 15 次請求）完全沒問題，但修訂鏈會隨重取次數成長；P17 排程化之前值得
   改為帶條件的查詢。

另有一項**超出本 PR 範圍**的既有落差：AGENTS.md §0 仍寫 `Rust 1.77+`，而 `Cargo.toml` 的
`rust-version` 在 main 上已是 `1.89`（P09 使用的 `usize::is_multiple_of` 需要 1.87+）。
這是 main 既有的文件過時，依 AGENTS.md §2 僅提出、不在此 PR 修改。

## 尚未證明的部分（維持 draft 的理由）

- **未進行任何一次真實 Tiingo 認證請求**。免費方案對 EOD 與 distributions 的實際權限、
  真實付款日涵蓋率、真實修訂行為，全部只有合成 fixture，fixture 不能代替。
- 因此 P09 在 `tasks.md` 維持 **In Progress**，`docs/market-source-tiingo-v1.md` 的
  `OK` 語意仍只代表「本範圍資料前置檢查通過」。

## 下一步（給下一位 agent／操作者）

1. 操作者於 Windows 認證管理員新增一般認證：位址 `com.alphafactorforge.desktop/tiingo`、
   使用者名稱 `tiingo`、密碼填自己的 API token。不要貼進對話、設定檔或環境變數。
2. 以隔離 `--data-dir` 跑一次 2026 年內的小範圍，逐檔記錄 metadata、實際權限、原始涵蓋與
   公司事件可得性，並把證據追加到 `handoffs/2026-09-21-p09-tiingo-v1.md`。
3. 403 或缺付款日是**誠實的 degraded 結果**，不是升級方案或把 `corporateActionsConfirmed`
   改成 true 的理由。
4. 外部驗收完成後才把 P09 移到 Done，並由使用者決定 PR #111 的合併時機。P10 未獲授權。

## Resolution：遠端後續狀態（2026-09-21）

- 後續 fetch 證實 PR #111 已由外部操作於 `2026-09-21T11:35:08Z` 合併，merge commit 為
  `396ee568d88af3516d7bb73c6f30658165ea0488`；記錄此 Resolution 的 agent 未執行 merge。
- 合併 revision 的 GitHub CI 六項皆成功：typecheck、test、build、cargo-check、native-smoke、e2e。
- 合併後再次執行原生 `tiingo-status` 仍為 `credential_missing`（exit 5）。所以本文件原列的
  真實帳戶、免費權限、付款日與涵蓋率限制仍然成立；P09 外部驗收維持 In Progress，P10 未啟動。
- PR [#112](https://github.com/yoyoCadence/AlphaFactorForge/pull/112)（docs-only 同步）驗收：上列合併時間、merge commit、六項 CI 與
  合併後 `tiingo-status` 皆經獨立複核相符；依 `handoffs/README.md` 生命週期一併更新本文件的 Status 行，
  原審查結論與內文不變。
