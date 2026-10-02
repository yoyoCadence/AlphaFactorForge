# Handoff: PR #135 noise simulation acceptance review

Date: 2026-10-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e3-noise-simulation`
PR: [#135](https://github.com/yoyoCadence/AlphaFactorForge/pull/135)
Reviewed head: `37e5a4ccccf46afbdfeb1441e0b946b91db1e3f0`
Base: `b5a343840375179b91effff8b9c02cbd2f8e49fe` (merged #134)
Status: R1 and R2 fixed on the PR branch (2026-10-02, see Resolution); awaiting re-review/merge. The declared simulation results reproduce; P12e-FINDING-1 remains open. The recommendations below were adopted by the maintainer on 2026-10-02.

## Summary

CI 六項全部成功，本機完整測試也通過。兩組事前宣告的模擬重現為獨立噪音 **97/2000**、AR(1) 0.3 噪音 **129/2000**；計數、明細分配、跨批次 OR、整數門檻與小案例的 TypeScript/Rust parity 均符合合約。`a0cd10a` 已有驗收宣告而沒有結果；後續 fixture 變更保留宣告值並加入報告。

本次找到兩個新邊界問題：固定 64 根暖機不支持整個允許的 AR(1) 係數範圍；host 等待器在 deadline 加 grace 都過期後會丟失仍活著的服務 thread，後者稍後仍可取得工作區。兩項皆有額外探針重現，建議在合併前修正。這不改變原 AR(1) 0.3 的超標結果，也不是要求這個 PR 同時完成 P12e-1 新版統計檢定。

## Required Action / Decision

### R1 — P2: 暖機 64 根不足以保證所有允許係數的平穩狀態

位置：

- `alpha-factor-forge/src-tauri/src/discovery_core/noise_simulation.rs:37`：暖機常數與平穩狀態註解。
- 同檔 `:166`／`:257`：接受 `autocorrelationPpm` 從 0 到 999999。
- 同檔 `:356`：從零開始，只捨棄固定 64 根。
- `docs/research-noise-simulation-v1.md:83`：聲稱暖機已把序列放到平穩狀態。

對實作的 `x_t = φ x_(t-1) + e_t`，從 `x_0 = 0` 開始：

```text
Var(x_t) / Var(stationary x) = 1 - φ^(2t)
第一根保留值 t = 65
φ = 0.99       → 只有平穩變異數的 72.924574%
φ = 0.999999   → 只有平穩變異數的 0.012999%
```

獨立 Rust 探針確認 `simulate_noise` 接受 `autocorrelationPpm = 999999` 的合法小宣告，再以公開的 `NOISE_WARMUP_BARS` 計算第一根的變異數比例；平穩暖機斷言失敗，得到 `0.00012999161536153547`。現有測試只檢查 0／0.3 的長序列矩與固定捨棄步數，沒有驗證高係數時的初始分布。

請讓允許範圍與模型承諾一致：限制係數到能滿足明訂暖機誤差的範圍，或採用按係數計算、有成本上限的暖機規則／平穩初始化。若保留有限暖機模型，必須明示初始化偏差及適用範圍，讓後續 P13 無法把未平穩的案例當成平穩 AR(1) 校準證據。補高係數的邊界測試並同步 TypeScript 參考。

**原驗收結果應保留。** φ=0.3 時初始變異數殘差 `φ^130 ≈ 1.06e-68`，此問題不能解釋那組 6.45% 的結果。不要用它撤銷 P12e-FINDING-1，也不要事後改原宣告或門檻。

### R2 — P2: 等待與寬限都逾時後，服務仍被留在背景

位置：`alpha-factor-forge/src-tauri/src/discovery_runner/tests/host.rs:203`。

`await_publication` 的 `None` 分支只產生 `is left running` 訊息並 panic，沒有停止服務；傳入的 JoinHandle 被丟棄，Rust thread 因而脫離管理。現有 never-published 測試也斷言這個訊息，之後由測試自己 `drop(release_tx)`，沒有覆蓋真服務在失敗後發布並持有工作區的情況。

重現方式：在 host 測試模組暫時加入一個探針，先用 channel 阻住真實 `service::run`，以 20ms deadline + 20ms grace 呼叫 `await_publication` 並捕捉其 panic；然後開放該 thread 啟動。

實際結果：

1. Helper 回報 `it still had not published 20ms later and is left running`。
2. 已被判定啟動失敗的服務仍然發布端點並取得工作區 epoch 1。
3. 只有探針自己的 `service::stop` 才使它停止；探針等待服務回傳並確認工作區 lease 已釋放，然後令「失敗後不得再持有工作區」斷言失敗。

探針指令：

```powershell
cargo test --locked --bin alpha-factor-forge pr135_review_timeout_must_not_leave_a_future_service_owner -- --nocapture
```

請讓失敗後的服務仍有受控的清理責任，不能直接丟掉 handle。對不可取消的 in-process 啟動，考慮可被終止並等待退出的隔離測試程序；或採可合作取消且保留管理權的測試啟動控制。保留有限等待、原始失敗原因與正式逾時。補真實服務晚於 deadline + grace 的測試，確認測試失敗後沒有 endpoint／服務／workspace lease 遺留。單純把 30 秒再拉長不能修正此分支。

以上是測試支援程式的問題，不是正式服務執行路徑的變更要求。

## Recommendations for P12e-FINDING-1

### 建議選擇方向 1，方向 2 作為經驗驗證後的適用範圍

保留本次宣告與兩組結果；P12e 維持未結案，P13 的確認／PASS 不進入實作。下一個小任務先宣告新版校準計畫，再調查／比較有處理序列相關的 studentized 統計量或變異數修正，作為 P12e-1 新合約版本的候選。這是研究方向，尚未證明它會修好偏差。

更長樣本可以是最後的適用條件，但須由事前宣告的多個樣本長度與噪音模型驗證，不能先猜最低 bars。每次宣告重跑模擬也只能檢查所宣告的合成模型；不能取代厚尾、波動叢聚或真實資料適用性的驗證。

不建議以「接受偏差後直接降低 alpha」作為結案方式：一個配置下的名目／觀察比率不是普遍有效的校正係數；如果採校準後的有效 alpha，仍須新版規則、獨立的事前驗收與所有明細份額的驗證。

### 判斷這次的結果時，保留抽樣不確定性

依 [NIST 的 Wilson 信賴區間公式](https://www.itl.nist.gov/div898/handbook/prc/section2/prc241.htm)，本次獨立計算以下 95% 區間（以 N=2000 的模擬計數為輸入；對每一列個別計算，未作同時多重比較校正）：

| 測量 | 比率 | 95% Wilson 區間 |
| --- | --- | --- |
| 獨立噪音，整個家族 97/2000 | 4.85% | 3.99%–5.88% |
| AR(1) 0.3，整個家族 129/2000 | 6.45% | 5.45%–7.61% |
| 獨立噪音，第 1 次 61/2000 | 3.05% | 2.38%–3.90% |
| AR(1) 0.3，第 1 次 83/2000 | 4.15% | 3.36%–5.12% |

原先的點估計門檻是事前決定，**129 > 120，這次驗收仍然失敗**。區間支持針對名目 5% 的校準疑慮，同時包含 6%；不能把它改述成「已證明真實機率超過 6%」。獨立噪音第 1 次的區間包含 2.5%，也不宜單憑 61 次就定論獨立噪音的檢定已失準。

不要事後用這些區間改成本次的驗收門檻。新計畫可以事先決定信賴上界門檻及容忍度／樣本數，並保留原點估計，讓誤判率控制與量測不確定性都可審核。

### 新版校準計畫的最小要求

- 先記錄欲控制的單次確認與整個家族目標、模型、樣本長度、候選／familyTests、明細份額、block 規則、bootstrap 數、Monte Carlo 規模、seed 列表與驗收規則，commit 後才跑。
- 診斷／方法選擇與最後驗收使用不同的事前固定 seed 集合；保留失敗，避免反覆選 seed 或 block 直到過門檻。全部使用合成資料，Validation／Test 不參與校準。
- 同時評估每次確認和整個家族；第二次因更大的 familyTests 而保守，可能掩蓋第一次超過其 alpha。家族通過不足以證明每份 alpha 都校準正確。
- 至少包含獨立與序列相關噪音，並把厚尾／波動叢聚列入後續支持範圍。預先控制矩陣規模與成本，先完成一個可驗收的小版本。
- 事後 L=6/11/16 的探索只能形成假說，尚不能排除 block 選擇或證明問題一定來自未 studentize。文獻也指出最佳 block 長度依估計目標而異；立方根不是所有單尾檢定的通用保證。[Hall、Horowitz、Jing（1995）](https://academic.oup.com/biomet/article-abstract/82/3/561/260651)

## Recommendations for the screenshot's CI questions

- 實際日誌確認兩組模擬所在 library suite 合計 **8.17s**，desktop suite **135.25s**；532 項通過與 1 項 ignored 的數字一致。保留這兩組正常 CI 中的重現測試；printer 是 ignored 不影響實際模擬的執行。
- 這一輪的通過不足以證明 30s 已解決 runner 偶發啟動慢。保留正式 timeout，服務啟動等待與整個 desktop suite 耗時分開記錄；先完成 R2 的清理，再收集成功／失敗啟動的耗時與階段證據。能穩定重現爭用時，再以事前測試方案評估 concurrency 或 timeout。
- PR body 仍有未勾選的遠端 CI 項目及舊成本估計；補上這輪 CI 連結與量測即可，屬非阻擋性的說明更新。

## Design-choice review

接受明細合計作為 nominal、家族 OR 的計數、明確的合成噪音與獨立 benchmark、事前固定的驗收設定、沒有 PASS 的狀態、PRNG visibility 的局部調整，以及大案例由 Rust 重現／小案例由獨立參考計算的分工。Family-only gate 可接受為本次宣告的量測規則；P12e-1 新版的有效性驗收仍須核對單次份額。模型適用範圍與 host 清理依 R1/R2 修正。

## Verification

本機獨立執行於 reviewed head：

- `cargo test --locked`：**532 passed（136 library + 394 desktop + 2 service），1 ignored**；兩組宣告模擬的 committed reports 完整重現。
- `npm test`：**1017 passed，59 files**；使用先前核准的沙箱外方式執行，以避免 esbuild 子程序的沙箱 EPERM。
- `npm run typecheck`、`npm run build`、`cargo check --locked --all-targets`：通過。
- 新 Rust 模組／測試的 `rustfmt --check`、`git diff --check`：通過。
- Reviewed head 的 [CI run 36999542085](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36999542085)：六項全成功；backend 日誌核對通過／ignored 數與耗時。
- 額外探針兩項都重現問題：R1 的平穩暖機斷言失敗；R2 真實服務在 helper 超過 deadline + grace 後仍能發布並持有 workspace。R2 探針在斷言前自行停止服務、等待結果並確認 lease 釋放。
- 暫時新增的 host 探針已按原始 bytes 還原，scratch Rust 探針與暫存目錄已刪除；產品程式／測試檔沒有永久變更。
- 作者的 mutation checks 與 clippy 沒有在本次重跑；本機未重跑 Playwright，遠端 e2e 成功。

本次只新增 review handoff 與 task board 記錄，未修正產品程式、未採用上述政策建議，也未 commit／push／張貼 GitHub review 或合併 PR。

## Resolution (2026-10-02)

R1 and R2 were acted on in PR #135, on top of the reviewed head `37e5a4c`.
This handoff and its task board lines were committed unchanged first
(`d97a78b`).

- **R1.** The coefficient is bounded instead of the warm-up changed:
  `autocorrelationPpm ∈ [0, 900000]`, where the first kept bar is short of the
  stationary variance by about 1.1 ppm. The fixed 64-bar warm-up and both
  declared acceptance runs are untouched (reports byte-for-byte the same).
  Contract §3 now states the initialisation bias and the supported range and
  refuses larger coefficients; a boundary test covers 900000 / 900001 /
  990000 / 999999 analytically and by measurement over 40,000 series; the
  fixture has a case at the maximum and the TypeScript reference enforces the
  same range.
- **R2.** A failed wait no longer drops a live service. The workspace is
  fenced with its own lock when free, and a cleanup thread stops a service
  that publishes late, joins it, releases the lock and removes the directory.
  The reviewer's scenario — a real service released only after deadline plus
  grace — is now a test: the service is refused, and no endpoint, directory or
  lock remains, without the test stopping anything itself. A second test
  covers a service already starting when the wait fails. The wait stays
  finite, the failure message and production timeouts are unchanged, nothing
  is retried. The in-process approach was kept rather than a separate
  terminable process; its remaining limit (a service that hangs without ever
  publishing or exiting keeps its thread until the process ends) is recorded
  in the P12e-3 handoff.
- **Recommendations.** The maintainer adopted them on 2026-10-02: revise the
  statistic, starting with a new calibration plan and then comparing a
  studentized bootstrap with a serial-correlation variance correction; keep
  the 6.45% failure on record with P12e open and P13 blocked; make the new
  acceptance check each confirmation and the whole family; keep the
  simulations in the normal suite and treat 30 s as a candidate until startup
  timings are collected. The Wilson intervals and their reading are in
  contract §7.1, and the over-strong wording in the P12e-3 handoff is
  corrected in its Resolution. The plan's minimum requirements are carried
  into contract §10 and task `P12e-4`.
- **PR description.** Rewritten: CI run linked, cost corrected to 8.17 s.

Verification after the fixes: 535 Rust (137 + 396 + 2, 1 ignored), 1019
Vitest, typecheck, build, all-target check and clippy (five existing
warnings) pass.
