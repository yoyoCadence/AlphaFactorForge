# P09 Tiingo 美股 ETF 日線來源

Plan：[`plans/active-plan.md`](plans/active-plan.md) P09。此 phase 僅接入
SPY、QQQ、VTI、TLT、GLD 的歷史日線；P10 台股、P17 排程、P18 共用成交核心、
P20 forward paper、P21 設定 UI 不在本次範圍。測試與真實帳戶驗證狀態見
[`../handoffs/2026-09-21-p09-tiingo-v1.md`](../handoffs/2026-09-21-p09-tiingo-v1.md)。

## 設定與執行

1. 註冊 [Tiingo](https://www.tiingo.com/about/pricing) 帳戶，取得自己的 API token。
   有免費 Starter 方案，但不要由方案名稱推定所有公司事件權限；程式會回報實際
   401／403／429，不會購買額度、升級方案或改用其他來源。
2. Windows「控制台 → 認證管理員 → Windows 認證 → 新增一般認證」：位址
   `com.alphafactorforge.desktop/tiingo`、使用者名稱 `tiingo`、密碼填 API token。
   更新／刪除亦使用此 OS 介面。不要把 token 放進 JSON、命令列、環境變數或 Git。
3. 複製 [`../alpha-factor-forge/config/tiingo-2026.example.json`](../alpha-factor-forge/config/tiingo-2026.example.json)
   至本機設定檔。範本只有非秘密設定；公司事件完整性未確認、成本為 `null`，所以
   即使行情齊全也應得到 `DEGRADED`。確認實際來源證據後才修改相應欄位。
4. 在 `alpha-factor-forge/src-tauri` 建置並執行：

```powershell
cargo build --locked --bin alpha-factor-forge-service
.\target\debug\alpha-factor-forge-service.exe tiingo-status
.\target\debug\alpha-factor-forge-service.exe fetch-tiingo `
  --settings ..\config\tiingo-2026.example.json `
  --from 2026-09-01 --to 2026-09-19 `
  --data-dir "$env:LOCALAPPDATA\AlphaFactorForge\p09-verification"
```

`--from` 包含、`--to` 不包含，都是交易所日期標籤，最多 366 個日曆日。
只接受已完成歷史範圍：最後日期隔天 06:00 UTC 後才准下載，保守涵蓋供應商
美東 20:00 前的修訂窗口。不宣稱已排除供應商日後修訂，也不提供即時價格。
不接受 `--token`。`tiingo-status` 只列存在與錯誤碼，不回傳秘密。
非 Windows 回報 `credential_platform_unsupported`，不降級為明文。

與既有 `fetch` 一樣，命令持有 P03 workspace lease，無法與桌面或服務同時寫入
同一 DB。先停止既有 owner，或用隔離目錄。沒有自動背景排程。
五檔各有一筆 JSON 報告；全部取得資料資格才 exit 0，任一 blocked/degraded 為 5；
workspace 被占用為 2；設定／IO 失敗為 1，CLI 語法錯誤為 64。

## 設定契約

`tiingo-settings-v1` 嚴格拒絕未定義的頂層及 instrument 欄位；檔案上限 256 KiB。
每檔設定包含 P08 `EtfMarket`、來源的 `exchangeCode`、上市界線與證據、
calendar 證據、公司事件完整性確認及可選的 P08 `EtfCostProfile`。
允許的映射是 `NYSE ARCA → nyse-arca`、`NASDAQ → nasdaq`，幣別只允許 USD。
不同名稱、重複代號、幣別或 venue 不符都拒絕。缺設定的預設代號仍列出報告。

範本收錄 2026 年日線 calendar，來源是 [NYSE](https://www.nyse.com/trade/hours-calendars)
與 [Nasdaq](https://www.nasdaqtrader.com/Trader.aspx?id=Calendar)（2026-09-21 核對）：
含 10 個休市日與 11/27、12/24 提早收市。早收仍有一根日線，未推算開收盤瞬間。
有效範圍僅 2026 年；超出範圍拒絕，不把週末以外的日期全部當成交易日。
臨時停市或來源變更須更新證據及 calendar 版本；已登記的停牌／下市／calendar
與設定不一致時回報 `registered_market_requires_reconciliation`，不默默移除。
`sessionsPerYear: 252` 是明示的波動年化設定，不代表每年確有 252 根行情。

範本上市界線依各發行商資料；其 URL 保存在 `listingEvidence`。
Tiingo metadata `startDate` 是**資料涵蓋起點**，不得拿它截短請求、冒充上市日期。
metadata 與實際 EOD 會分別核對要求的完整交易日範圍。既存更完整的 lot/tick
規格保留，本 adapter 不猜成交單位。

`corporateActionsConfirmed` 是操作者對設定證據期間之公司事件完整性的確認，
不能單由 HTTP 200、AI 建議或「這段沒有股息」而改為 true。確認仍不會繞過
公司事件端點拒絕、EOD／事件不一致、缺付款日及未確認成本等檢查。
成本設定使用 P08 原幣費用契約；無預設稅率，也未納入個人所得稅。

## 下載、保存與核對

每檔依序請求 [Tiingo 官方 EOD](https://www.tiingo.com/documentation/end-of-day)
的 metadata、raw/adjusted daily prices，以及
[distribution endpoint](https://www.tiingo.com/documentation/corporate-actions/dividends)。
最多 15 次 HTTP GET，每個 30 秒、8 MiB、一次嘗試；401／429 停止後續 batch 請求。
403 按 endpoint／商品分別報告。無不透明 retry、免費配額假設或付費 fallback。
只能 HTTPS 到 `api.tiingo.com`，不跟 redirect；token 僅放 Authorization header。

- 每個實際回應先保留原件，包含無效 JSON／被拒絕的行情與公司事件。
  raw、adjusted、divCash、splitFactor、未知擴充欄位均留在原件。
  HTTP 失敗只保存本機錯誤收據；不假裝拿到了原始 response body。
  provider 錯誤 body、認證 header、HTTP debug error 不寫入 DB／log／artifact。
- 重取資料會延伸相同 endpoint／期間的修訂鏈，包含 byte-identical 重取；
  所以之後更正能使所有前序觀測失去最新資格。原件不覆寫，沒有隱藏 cache。
- `availableAt` 取 response 完成時間；歷史 ex-date／公告日不冒充觀測時間。
  本次建立的 snapshot 僅為 `historical`，不能直接當作 forward 資料。
- 嚴格核對 raw／adjusted OHLCV 有限值、價位關係、整數股數成交量、正分割因子、
  非負股息、UTC 日期標籤、排序／重複／請求範圍。
- 整個請求先經 P06 coverage audit，首尾缺漏、週末／休市多出行情均不能建立
  dataset/snapshot；不剪掉缺漏、不補值、不拼其他來源。
- EOD 的每一筆正 `divCash` 與 distribution 雙向對照代號、ex-date、金額；
  重複、多出、漏掉、取消或付款早於除息皆阻擋公司事件資格。
  缺付款日保持 `None`，不可變成除息日付款。
- 分割轉為 P08 `SplitAction`；原始行情作 dataset，供應商 adjusted 僅供稽核，
  絕不代替原始成交價或變成帶有未來事件的訊號序列。
- 使用既有 P06 registry／provenance／snapshot 和 P05 content-addressed artifact
  store；完整每檔報告以 `tiingo-report` comparison provenance 保存，附設定及
  所有原件 ID。資料庫讀者可重開原件與 report，無新增 migration。

## 資格與限制

| 狀態／原因 | 結果與下一步 |
|---|---|
| `credential_missing/unavailable` | 不發網路請求；檢查 OS 一般認證 |
| `authentication_required` | 帳戶/token 不可用；更新認證後再明確執行 |
| `entitlement_denied` | 本端點權限不足；核對免費帳戶實際權限 |
| `quota_exhausted` | 停止 batch；等待來源允許後再手動執行 |
| `metadata_*`／`requested_range_incomplete_or_off_session` | 無合格 snapshot；核對來源範圍／calendar |
| `distribution_*`／`unknown_payment_date` | 保留原始行情，snapshot 為 DEGRADED |
| `corporate_actions_unconfirmed`／`costs_unconfirmed` | 完成來源完整性／成本確認前不能取得資格 |
| `OK` | 本範圍資料前置檢查通過；**不等於策略獲利、Test 合格或 paper 開戶授權** |

舊 dataset/hash/backtest、365 年化及 AI 秘密命令保持原契約；本階段不把 ETF
P08 計算強行接入既有 discovery 引擎。報告中的 split/dividend 可供 P18 消費。
這不是完整 ETF universe，也不推論多年資料、跨來源一致性或帳戶付費資格。
真實免費帳戶與公司事件完整性必須以實際回應驗證，fixtures 不能代替。

## 驗證入口

Rust 測試使用合成 EOD／公司事件與真 SQLite、artifact store；包括 normal flow、
missing/extra/duplicate、拒絕權限、配額、未知付款日、修訂及原件重讀。
既有 P08 TS／Rust shared parity fixtures 保持通過。
執行 `cargo test --locked`、`cargo check --locked --all-targets`、`cargo clippy --locked --all-targets`，
以及專案 `npm.cmd test`、typecheck、build、e2e。完整結果以本次 handoff 為準。
