# Handoff: PR #106–#115 整合驗收（往前驗收 batch B）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受。各階段在合併前都有驗收與修正紀錄；本次針對它們較少碰到的高風險邊界做了重點檢查，程式碼沒有新的缺陷，但找到一項文件矛盾（B-R1，P3）。仍有三項 operator acceptance 未完成（見下）。

## Required Action / Decision

### B-R1 — P3：文件寫的 Rust 最低版本與 `Cargo.toml` 矛盾

- 來源：PR #106（P03a，commit `e8f6934` 把 `alpha-factor-forge/src-tauri/Cargo.toml:8` 改成 `rust-version = "1.89"`）。
- 文件仍寫舊版本：`README.md:148,330,502`（三語都寫 Rust 1.77.2+）、`alpha-factor-forge/README.md:39`（≥ 1.77）、`AGENTS.md:12`（Rust 1.77+）。
- 影響：照文件準備 1.77–1.88 工具鏈的人，`cargo build` 會直接因 `rust-version` 被拒；AGENTS.md 是給所有 agent 的協作契約，寫錯版本也會誤導 agent。
- 修正：把上述文件改成 1.89（或改寫為「見 `Cargo.toml` 的 `rust-version`」）。AGENTS.md 屬於協作契約，請維護者決定措辭。
  既有的 `TOOLCHAIN-001`（P3，在 CI 證明 MSRV）是相關但不同的任務：那是「證明」，這裡是「文件已經和設定矛盾」。

## Summary

這十個 PR 是研究執行環境（P00–P05）、市場資料（P06–P10）、可執行 DSL（P11）與精度預檢（P12a）。
十個合併 commit 的 tree 都與各自的最終 head 相同，十個 head 的六項 CI 都通過。
每個階段都已有驗收紀錄，而且其中的發現都已修正並附回歸測試，所以本次不重做逐行審查，
改為檢查 AGENTS.md 列出的高風險面在**目前的 main** 上是否仍成立。

## 本次重點檢查（目前 main 上的程式碼）

| 面向 | 來源 PR | 結果 |
| --- | --- | --- |
| 本機 HTTP 控制介面（服務） | #106（P04a） | 只綁 `127.0.0.1`；`Host` 必須是本機 loopback 加正確 port（擋住 DNS rebinding）；有瀏覽器 `Origin` 一律拒絕，而且在驗證 token 之前；bearer token 以 constant-time 比對；body 有上限（`runtime/control_api.rs:7–21,338–345,672–708`）。 |
| 工作區擁有權 | #106（P03a） | 所有寫入都經 `write_transaction(_quiet)`，並在同一筆 `BEGIN IMMEDIATE` 交易內比對 epoch（`db/ownership.rs:135–171`）；production 程式碼沒有任何一處以 `None` 跳過檢查。 |
| 市場資料憑證 | #111（P09）、#113（P10） | Tiingo token 只由 Rust 從 Windows Credential Manager 讀取；沒有 CLI 參數、環境變數、檔案或前端命令，格式也有檢查（`market/tiingo_credentials.rs`）。FinMind／TWSE 不需要憑證。符合 AGENTS.md §10。 |
| DSL 的邊界 | #114（P11） | DSL 只是資料，以樹狀直譯器求值；有深度、節點數與因果性限制；`discovery-config-v2` 在列舉與寫入前驗證，執行時再驗一次。 |
| 精度預檢 | #115（P12a） | 先前驗收的 R1（超出範圍的數值溢位）已在合併前修正；之後被 #133、#140 沿用，那兩個 PR 的驗收也檢查過整數邊界。 |

## 依賴的既有驗收

| PR | 階段 | 驗收紀錄 | 結論 |
| --- | --- | --- | --- |
| [#106](https://github.com/yoyoCadence/AlphaFactorForge/pull/106) | P00–P04 | [P01 驗收](2026-09-17-p01-acceptance-review-v1.md)；P04a、P04b 的驗收修正記在 [P04a](2026-09-17-p04a-headless-service-v1.md)、[P04b](2026-09-18-p04b-desktop-connect-v1.md) handoff 與 tasks.md Done | 可接受；附 B-R1（文件） |
| [#107](https://github.com/yoyoCadence/AlphaFactorForge/pull/107) | P05 研究歷史 | Codex 的兩項發現已修正（[handoff Resolution](2026-09-19-p05-research-history-v1.md)） | 可接受 |
| [#108](https://github.com/yoyoCadence/AlphaFactorForge/pull/108) | P06 市場資料基礎 | [P06 驗收](2026-09-20-p06-acceptance-review-v1.md)，四項發現已修正 | 可接受 |
| [#109](https://github.com/yoyoCadence/AlphaFactorForge/pull/109) | P07 Binance | [PR #109 驗收](2026-09-20-pr109-p07-acceptance-review-v1.md)，兩項修正與回歸 | 可接受 |
| [#110](https://github.com/yoyoCadence/AlphaFactorForge/pull/110) | P08 ETF 語意 | [P08 驗收](2026-09-20-p08-acceptance-review-v1.md)，沒有 P1／P2 | 可接受 |
| [#111](https://github.com/yoyoCadence/AlphaFactorForge/pull/111) | P09 Tiingo | [P09 驗收](2026-09-21-p09-acceptance-review-v1.md)，沒有 P1／P2 | 可接受；外部驗收未完成 |
| [#112](https://github.com/yoyoCadence/AlphaFactorForge/pull/112) | 文件 | — | 可接受 |
| [#113](https://github.com/yoyoCadence/AlphaFactorForge/pull/113) | P10 台灣 ETF | [P10 驗收](2026-09-21-p10-acceptance-review-v1.md)，沒有 P1／P2 | 可接受 |
| [#114](https://github.com/yoyoCadence/AlphaFactorForge/pull/114) | P11 DSL | 驗收後的追加 commit `22c8739`（tasks.md） | 可接受 |
| [#115](https://github.com/yoyoCadence/AlphaFactorForge/pull/115) | P12a 精度 | [PR #115 驗收＋覆驗](2026-09-23-pr115-acceptance-review-v1.md)，R1 已修正 | 可接受 |

## 仍未完成的 operator acceptance（不是缺陷，但不要當成已驗證）

1. **P01**：native Tauri 的重啟／重開路徑沒有測過（P01 驗收的 Status）。
2. **P09**：需要真實 Tiingo 憑證的外部驗收還沒做（tasks.md 記為 P09 still needs authenticated acceptance）。
3. **服務的 token 檔**：token 存在工作區資料夾的 `control-token` 檔（`runtime/control_api.rs:46,153`），程式沒有另外設定 ACL；P04b 明確把 Windows ACL 列為範圍外。
   保護完全取決於資料夾的預設權限：預設的使用者 AppData 只有該使用者、SYSTEM 與管理員可讀；但若用 `AFF_DATA_DIR` 指到共用位置，就沒有保護。要支援共用位置前需要另外處理。

## Verification

- 十組 `git diff <head> <merge>` 都是空的；十個 head 的六項 CI 都是 SUCCESS（runs：35406102547、35454791809、35485112915、35500938087、35510972639、35593606965、35596929477、35604499351、35733301726、35856386609）。
- 全套測試沿用整合 baseline 的結果（566 Rust、1072 Vitest、82/82 Playwright 等，見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。
- 本次沒有新增反例：上表的檢查都是讀程式碼確認。B-R1 以 `grep` 比對 `Cargo.toml` 與文件確認，並以 `git log -S` 找到引入的 commit。

## Resolution

Pending for B-R1 (documentation). The operator acceptance items above are informational.
