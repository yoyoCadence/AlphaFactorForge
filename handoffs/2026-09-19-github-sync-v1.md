# Handoff: GitHub 版本控制同步恢復

Date: 2026-09-19
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`
PR: https://github.com/yoyoCadence/AlphaFactorForge/pull/106 (draft; base `main`)
Status: 本機成果與原始歷史已推送；草稿 PR 已建立，尚未合併

## Summary

使用者確認 GitHub 解鎖並要求將本專案 Git 全部放到 GitHub。開始時有 20 筆本機提交尚未在遠端，
另有 P04b H2 修正與一份既有 P03b 交接筆記未提交。先分開保存這兩組變更，再將 22 筆提交
rebase 至 `origin/main` `97eaf9e`（PR #105 的 atomic report writer）。只在 tasks／changelog 發生衝突，
保留兩邊紀錄；產品變更自動整合，報表 writer 與 main 完全一致。

## Published branches

- `docs/p00-contract-precheck`：P00–P04 及驗收修正，H2 修正 rebase 後為 `9af8a21`。
- `archive/local-before-github-sync-2026-09-19`：`dbb4279`，完整保留 rebase 前的提交編號，舊交接引用仍可查。
- `docs/alphabtc-capability-transfer`：原始能力轉移規格分支。
- `fix/theme-contrast-dead-surface`：原本尚未發布的本機歷史分支。

四個新分支以 atomic push 一次發布並設定 upstream。其餘遠端分支保留，不 force-push、不刪除、未 merge。
本機 main 已 fast-forward 至 `97eaf9e`。首次推送後檢查所有 24 個本機分支，
`git rev-list --count --branches --not --remotes=origin` 為 **0**：所有分支可達的本機提交均有遠端保存。
沒有本機 tags 待同步。既有 `.gitignore` 維持不變，建置產物、本機資料庫與私密檔案不納入版本控制。

## Verification after rebase

- `cargo test --locked`：**250 passed（52 + 196 + 2）**；比 H2 驗收多 3 項，來自 main 的報表輸出測試。
- `cargo check --locked`：通過，0 warning。
- `npm.cmd test`：**879 passed**；`npm.cmd run build`（含 typecheck）通過。
- Playwright host-mode **6 passed**；export 首次在開頁階段超過 30 s，單獨以
  `--timeout=120000` 重跑 **1 passed**（42 s），未修改測試原始碼。
- rebase 前完整 75 項瀏覽器案例已有通過紀錄，詳見 P04b handoff；本次 rebase 後未重跑其餘案例。
- 原生 Tauri 視窗操作未重跑；GitHub CI 由 PR 執行，尚未宣告 CI 通過。P05 未開始。

## Publishing environment

Git 的 GitHub credential helper 使用 GitHub CLI；注入的 `GITHUB_TOKEN` 已失效，會蓋過電腦既有的
keyring 登入。只在這次 Git 命令執行期間移除該環境變數，finally 還原，fetch／push 隨即成功；
未列印 token、未修改永久認證或環境設定。

GitHub connector 建立 PR 仍回覆 403 `Resource not accessible by integration`。依
`open-pull-request`／Chrome skills 使用既有登入的 Chrome 建立並確認 draft PR #106，
base `main`、head `docs/p00-contract-precheck`。既有同 head PR 搜尋為空，未建立重複 PR。

## Next

PR #106 的 CI 與審閱完成後再決定合併。本次授權是保存與發布版本，未變更 main 的產品內容。
