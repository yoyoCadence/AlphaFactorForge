# Handoff: PR #46–#55 整合驗收（往前驗收 batch H）

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `review/pr001-130-acceptance`
Reviewed baseline: `5b4a562`
Status: 驗收完成；十個 PR 都可以接受。程式碼沒有新缺陷；但 #53／#54 建立的「npm audit 為零」狀態已經不成立（H-R1，P3，全部是開發工具依賴）。

## Summary

這十個 PR 是回測 `both` 反手語義與核心輸入驗證（#46）、原生績效視窗（#47）、重新保存策略時更新可變欄位（#48）、
程式碼條件無效時停用回測（#49）、兩個 e2e（#50、#51）、交易明細與摘要一起寫入 SQLite（#52）、
npm audit 盤點與 Vite 6／Vitest 3 升級（#53、#54），以及 Train／Validation／Test embargo 切分契約（#55）。
十個合併 commit 的 tree 都與各自的最終 head 相同，所有 head 的 CI 都通過。這十個 PR 都沒有 PR 審查留言。

它們的核心行為之後都被更嚴格地鎖住：回測語義由 Rust parity（#69，Codex 三輪驗收）與本次 batch F 的 3,600 例差分測試（其中 209 例是 `both`）確認一致；
交易明細的寫入由 #104 加上經過審查的一致性檢查；切分契約由 #70 的 parity 與 #89 的嚴格驗證沿用。

## Required Action / Decision

### H-R1 — P3：`npm audit` 又出現 6 個發現（3 high、3 moderate），全部在開發工具依賴

- 背景：SEC-002（#54）在 2026-07-16 把 `npm audit` 與 `npm audit --omit=dev` 都清到 0（`docs/security-audit-npm.md`）。之後有新的 advisory 公布。
- 現況（2026-10-04，`alpha-factor-forge/`）：

  | 套件 | 嚴重度 | 直接／間接 | 是否有不跨 major 的修正 |
  | --- | --- | --- | --- |
  | postcss | high | 間接（Vite 建置） | 有 |
  | browserslist | high | 間接 | 有 |
  | nanoid | high | 間接（postcss） | 有 |
  | baseline-browser-mapping | moderate | 間接 | 有 |
  | @vitest/mocker | moderate | 間接 | 需要 Vitest 5（major） |
  | vitest | moderate | 直接（`3.2.6`） | 需要 Vitest 5（major） |

- `npm audit --omit=dev`：**0**，production bundle 不受影響。
- 影響：這些都在開發與建置時執行，且處理的是本專案自己的原始碼，而非不受信任的輸入，所以實際風險低；
  但 `docs/security-audit-npm.md` 的驗收條件是「兩種 audit 都為零」，目前不成立。
- 修正方向：依該文件的規定處理——**不要執行 `npm audit fix`／`--force`**，改以明確版本升級並逐行審查 lockfile；
  四個可在範圍內修正的間接依賴可先處理；Vitest 5 屬於 major 升級，需要另開任務並跑完整 typecheck／unit／build／e2e。
  在處理完之前，文件中的「暫時控制」（不以 `--host` 啟動 Vite、不開 Vitest UI／Browser Mode、用完即關閉本機伺服器）重新適用。
  也可考慮在 CI 加一個 `npm audit --omit=dev` 檢查（目前為 0，可作為 production 的守門）。

## 本次其他檢查

- #48：`insert_strategy` 以 `strategy_hash` 做 UPSERT，衝突時只更新 `name`、`source` 這類非身分欄位；偽造或舊版的 hash 會被 `verified_strategy` 拒絕（有測試）。
- #46：`both` 的反手語義與輸入驗證，見上方的 parity 與差分測試。

## Per-PR acceptance

| PR | 內容 | 結論 | CI run |
| --- | --- | --- | --- |
| [#46](https://github.com/yoyoCadence/AlphaFactorForge/pull/46) | `both` 反手語義、核心輸入驗證 | 可接受 | 29340257843 |
| [#47](https://github.com/yoyoCadence/AlphaFactorForge/pull/47) | 原生績效視窗 | 可接受 | 29343023249 |
| [#48](https://github.com/yoyoCadence/AlphaFactorForge/pull/48) | 重新保存時更新可變欄位 | 可接受 | 29420869656 |
| [#49](https://github.com/yoyoCadence/AlphaFactorForge/pull/49) | 程式碼條件無效時停用回測 | 可接受 | 29423162604 |
| [#50](https://github.com/yoyoCadence/AlphaFactorForge/pull/50) | e2e：策略模式切換 | 可接受 | 29494878939 |
| [#51](https://github.com/yoyoCadence/AlphaFactorForge/pull/51) | e2e：儲存成功訊息 | 可接受 | 29496077241 |
| [#52](https://github.com/yoyoCadence/AlphaFactorForge/pull/52) | 交易明細寫入 SQLite | 可接受（之後 #104 加上一致性檢查） | 29498219305 |
| [#53](https://github.com/yoyoCadence/AlphaFactorForge/pull/53) | npm audit 盤點 | 可接受；現況見 H-R1 | 29500943317 |
| [#54](https://github.com/yoyoCadence/AlphaFactorForge/pull/54) | Vite 6、Vitest 3 | 可接受；現況見 H-R1 | 29502626784 |
| [#55](https://github.com/yoyoCadence/AlphaFactorForge/pull/55) | embargo 切分契約 | 可接受 | 29506000965 |

（CI run 連結格式：`https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/<id>`。）

## Verification

- 十組 `git diff <head> <merge>` 都是空的；各 head 的 CI 都是 success。
- `npm audit --json`：6（moderate 3、high 3）；`npm audit --omit=dev`：found 0 vulnerabilities。
- 全套測試沿用整合 baseline 的結果（見 [#131–#140 驗收](2026-10-04-pr131-140-acceptance-review-v1.md)）。

## Resolution

Pending for H-R1.

## Follow-up work order（2026-10-04 追加）

本檔的發現已整理進 [驗收後續工作單](2026-10-04-acceptance-followups-work-order-v1.md)，請接手的 agent 從那裡開始：

- H-R1 → **FU-3**，含每個套件的目前版本、引入路徑與需要的最低版本；需要 **D4**（Vitest 升到哪一版、是否另開 PR）與 **D5**（CI 是否加 `npm audit --omit=dev`）。

### Resolution — FU-3a（2026-10-04）

H-R1 的四個間接依賴已在 `chore/dev-deps-audit-2026-10` 於相容範圍內更新（postcss 8.5.28、nanoid 3.3.19、browserslist 4.29.3、baseline-browser-mapping 2.11.27；browserslist 連帶更新四個自身依賴），未使用 `npm audit fix`，`package.json` 不變，lockfile 逐行審查。`npm audit` 只剩 vitest／@vitest/mocker（交給 FU-3b 升到 4.1.11），`--omit=dev` 為 0；typecheck、1073 Vitest、build、82/82 Playwright 通過，dev server 仍只綁本機。紀錄見 `docs/security-audit-npm.md`。

### Resolution — FU-3b（2026-10-04）

Vitest 已在 `chore/vitest-4` 升到 4.1.11（GHSA-82fw-gwwq-j7x9 的最低修補版本），保留 fixture 指令使用的直接 `vite-node@3.2.4`；三個代表性 fixture 重新產生內容完全相同。`npm audit` 與 `--omit=dev` 都回到 **0**，H-R1 的依賴部分完成（CI 檢查 FU-3c 另行處理）。

### Resolution — FU-3c（2026-10-04）

CI 的 `typecheck` job 在 typecheck 之後加上 `npm audit --omit=dev`（只守 production 依賴，任何 advisory 或 registry 失敗都讓 job 失敗；devDependencies 仍依 `docs/security-audit-npm.md` 人工處理）。以暫時專案驗證：production 有漏洞 exit 1、只有 dev 漏洞 exit 0。H-R1 完成；定期完整 audit 另列 Backlog `SEC-NPM-AUDIT-SCHEDULE-001`。
