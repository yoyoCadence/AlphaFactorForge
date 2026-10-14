# Handoff: PR #169 NumberInput 失焦修正驗收

Date: 2026-10-14（Asia/Taipei）
Repo: yoyoCadence/AlphaFactorForge
PR: [#169](https://github.com/yoyoCadence/AlphaFactorForge/pull/169)
Branch under review: `fix/number-input-blur-commit`
Reviewed head: `d39e5c0d5822fd7f86a988fe5f3f5eeab9e8ef50`
Base: `07417268d1b964a9444409578e994c6da8fd4f3b`
Status: Accepted — 沒有新增阻擋合併的發現；驗收結束時 PR 仍 open，head 未改變。

## Summary

維護者要求驗收 PR #169。BUG-SWEEP-BLUR-001（專案審查 A1、A2）的修正符合範圍：
未改值的失焦不再觸發父元件的編輯處理，熱力圖、點格子／套用最佳及策略套用標記正常；
即時修改、清空後復原與失焦夾限維持原本行為。沒有需要此 PR 再修的 P1／P2／P3。

## Scope and method

- 比對 GitHub metadata／完整 diff 與本機 commit；變更共五檔，151 additions／11 deletions。
  產品程式只改 `NumberInput.tsx`，其他為八個 helper 單元案例、兩個 sweep E2E 與交接／任務紀錄。
- 檢查所有八處共用元件呼叫，以及父層 `clearSweep`、`setNum`、Holdout 與 Discovery 數值更新。
  新增的提交值計算維持原本 parse／fallback／clamp 順序；只在失焦提交值不同時回報。
- 當前共享工作區為 `fix/crypto-listing-registration`，有既有未提交變更。
  以 `git archive` 在獨立暫存目錄取出 reviewed head，依其 lockfile 離線安裝依賴後執行驗收，
  不把當前工作區或其他 worktree 的程式冒充 PR #169 的版本。
- 使用 React 與前端測試技能；Browser runtime 選擇 Chrome，URL 為 `http://localhost:5189/?mock=1`，
  桌面 viewport 1920 × 911。另執行專案既有 Chromium E2E。

## Verification

| 檢查 | 本輪結果 |
| --- | --- |
| `npm ci --offline --no-audit --no-fund` | 通過，依 reviewed head 的 lockfile 安裝 106 packages |
| `npm run typecheck` | 通過 |
| `npm test` | 71 檔、1139／1139 通過 |
| `npm run build` | 通過 |
| `E2E_PORT=5189 npm run e2e -- --workers=1` | 87／87 通過 |
| 新增兩個 E2E 的反向驗證 | 舊元件 2／2 預期失敗；恢復 PR 元件後 2／2 通過 |
| GitHub 最終 head CI | 六項全部 success，含 Rust 與 native smoke |
| 驗收結束再次讀取 PR | head 仍為 `d39e5c0`，open、mergeable |

最終 head 的 [CI run 38485684360](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38485684360)
包含 `typecheck`（亦含 production audit）、`test`、`build`、`e2e`、`cargo-check`（含 backend tests）、
`native-smoke`（含原生建置與 smoke），六項全部通過。Rust／native 結果來自本輪查核的 CI，沒有在本機重跑。

反向驗證只在隔離目錄以 base 的原始 `NumberInput.tsx` 取代 PR 元件：

1. 第一個新增 E2E 在未改值失焦後預期 16 格，實際 0 格（`sweep.spec.ts:135`）。
2. 第二個新增 E2E 在失焦後找不到 `applied-fastMA`（`sweep.spec.ts:164`）。
3. 恢復 PR 元件並核對備份／恢復檔的 SHA-256 完全相同，重跑同兩個 E2E 全數通過。

## Browser interaction evidence

流程：載入樣本資料 → 展開參數掃描 → 執行掃描 → 欄位失焦／套用 → 觀察格數、數值與標記。

- 一維 X 軸「起／迄／間距」逐一 focus → Tab：每次保留 16 格與套用最佳按鈕。
- 從 X 軸欄位直接點套用最佳：快線 MA = 7，熱力圖及套用標記保留。
- 從 X 軸欄位直接點值 6 的格子：快線 MA = 6，確認訊息與 16 格均保留。
- 套用後的策略欄位 focus → Tab：標記保留；清空後 Tab：數值復原為 6，標記與 16 格均保留。
- 手續費 0.05 未改值失焦：數值與 16 格保留。
- 二維共 176 格，X／Y 六個軸欄位逐一 focus → Tab：每次均保留 176 格。
  從 Y 軸欄位點套用最佳亦成功，產生一個套用標記；已截圖並在驗收會話顯示。
- 策略快線 MA 真正輸入 0：即時回報 0、移除套用標記，Tab 後依 min 夾限為 1。
- 真正把 X 軸起點 5 改為 6：格數變 0，套用最佳移除。
- 頁面 title／URL 正確、有完整內容、無框架錯誤 overlay。
  Console 在實際修改移除套用標記時有既有 `border`／`borderColor` React 警告，
  對應原審查 A10／`REACT-STYLE-WARN-001`，已排在下一個快修，不是此 PR 新增問題。

## Limits and next step

本輪互動使用 sample／mock client；沒有另做 Tauri 視窗互動、Safari／WebKit、手機版或受控延遲的
「掃描進行中失焦」測試。既有原生 CI 通過不等於這些互動都已獨立驗證。
這次沒有改 Rust、依賴、schema 或統計契約；上述限制不阻擋本 PR 的小範圍共用元件修正。

建議合併 reviewed head，接著依既有順序處理 A10／A11 的快修。
驗收不包含發佈 GitHub review／comment 或 merge；本輪沒有執行這些寫入。
已關閉驗收瀏覽器 tab，核對 port 5189 的 Vite process command line 後停止該 process，確認無 listener。
原有未提交產品變更保留；本輪工作區新增本文件及 `tasks.md` 的驗收完成紀錄。

## Resolution

Pending merge closure。合併後追加 merge SHA；若 PR head 改變，重新確認變更範圍與相關驗證。

### Resolution — 合併與整合確認（2026-10-14）

#169 已於 2026-10-14 合併為 `43eea868c8900f368828a592658f68fc316067c4`，最終 head 仍為 `d39e5c0`。
merge tree 與 reviewed head 相同；本輪查核其六項 CI 全通過。
在六項快修合併後的 `b08aa38`，本機 1170 Vitest／93 E2E、typecheck／build 亦通過。
本 PR 的驗收維持通過；[整批驗收與其他切片待修項目](2026-10-14-pr169-174-acceptance-review-v1.md) 另列。
