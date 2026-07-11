# AlphaFactorForge — Agent Execution Protocol（低成本 coding agent 標準作業流程）

> 撰寫日期：2026-07-07。
> 本文件與 `AGENTS.md`（協作契約）**相容且從屬**：AGENTS.md 定義「不可做什麼」，本文件定義「照什麼順序做」。衝突時以 AGENTS.md 為準。
> 任務規格來源：[improvement-backlog.md](improvement-backlog.md)。任務狀態唯一記錄處：`tasks.md`（AGENTS.md §8）。

---

## 1. Agent Roles

| 角色 | 職責 | 不做什麼 | 建議模型等級 |
| --- | --- | --- | --- |
| **Planner agent** | 讀 audit/backlog，挑下一個任務；必要時把大任務拆成 S/M 級規格（寫回 backlog 格式）；起草給 coding agent 的最終 prompt | 不寫程式、不改 repo（除了 docs/backlog 與 tasks.md 狀態行） | 高（本審計即 Planner 產物；日常可用中階） |
| **Coding agent** | 執行**一個** backlog 任務：開 branch → 實作 → 驗證 → PR | 不挑任務、不改規格、不做規格外的「順手改善」 | 低-中（任務規格已寫到可照抄） |
| **Reviewer agent** | 對照任務規格審 PR：scope、行為、證據；產出裁決 | 不直接改 code（發現問題退回，或另開 fix 任務） | 中（需要懷疑精神多於生成能力） |
| **User / maintainer** | 裁決 Open Questions；核准依賴/schema/行為變更；merge PR；驗收 Tauri 手動 smoke | —（所有不可逆決策的唯一擁有者） | 人類 |

角色可以由同一個人/同一個 session 兼任，但**同一個 PR 的 coding 與 review 不得是同一個 agent session**（自我審查無效）。

## 2. Standard Execution Flow

每個任務固定走這 7 步，任何一步失敗就停下回報，不得跳步：

```
1. Read docs      → AGENTS.md 全文 → docs/improvement-backlog.md 的目標任務 → 任務指到的其他文件
2. Confirm scope  → 用一句話覆述任務目標 + 列出「我會改的檔案」清單；與規格的 Files likely affected 對照，
                    超出清單的檔案 = 先停下來問
3. Git check      → git status -sb 必須乾淨；git fetch origin && 從最新 origin/main 開新 branch（命名見 §3）；
                    在 tasks.md 把任務加入 In Progress（一行 + backlog Task ID 連結）
4. Implement      → 只做這一個任務；跟著規格的 implementation plan 逐步做；
                    每完成一步自查該步的驗證點
5. Run validation → 依任務的 Validation plan 全跑（見 §2.1 標準指令）；貼出真實輸出，失敗不得掩蓋
6. Produce summary→ PR：英文 conventional commit 標題、zh-TW 內文（摘要／改了什麼／驗證清單／殘餘風險）、
                    git diff --stat、勾選 acceptance criteria
7. Stop for review→ 開 PR（或 draft）後停止。不自行 merge、不繼續下一個任務、不「趁機」加 commit
```

### 2.1 標準驗證指令

```bash
# 前端（所有任務必跑）
cd alpha-factor-forge
npm run typecheck
npm test
npm run build
npm run e2e          # 本機 Windows 用預設 workers=1，勿加 --workers 覆寫

# Rust（動到 src-tauri 時必跑）
cd alpha-factor-forge/src-tauri
cargo check --locked
cargo test --locked

# 手動 smoke（動到 Tauri command / DB 時，由 maintainer 或有桌面環境的 agent 執行）
cd alpha-factor-forge && npm run tauri -- dev
# DB 實況查驗一律用 sqlite CLI；GUI viewer 可能讀到 WAL 之前的舊快照
```

### 2.2 卡住時的處理

- 規格與現實衝突（檔案不存在、行為不同）→ **停**，在 PR/回報中描述差異，不要自行腦補繞過。
- 需要新套件、新 schema、行為變更而規格未授權 → **停**，回報並引用 masterplan 對應的 Open Question。
- 測試紅且 20 分鐘內查不出因 → 貼完整錯誤輸出回報，不要為了綠燈弱化測試。

## 3. Branch Rules

| Prefix | 用途 | 例子 |
| --- | --- | --- |
| `refactor/*` | 行為零變更的結構調整（move-only、rename、抽檔） | `refactor/extract-sweep-section` |
| `feature/*` 或 `feat/*` | 新功能／新 UI 區塊／新 command（本 repo 慣例是 `feat/`，沿用） | `feat/ui-port-slice7-3-strategy-library` |
| `fix/*` | 行為修正（bug、紀律破口、UX 缺陷） | `fix/sweep-respects-holdout` |
| `docs/*` | 只動文件 | `docs/status-single-source` |
| `test/*` | 只動測試（+最小 testid 類產品修改，需在 PR 列明） | `test/backtest-golden-lock` |

規則：

1. 一律從**最新** `origin/main` 開分支（`git fetch origin` 先行）；push 前若 main 前進，rebase 後重跑驗證（AGENTS.md §7）。
2. main 開啟「branch 必須 up to date 才能 merge」：**多個 PR 依序合併，別平行開一堆互撞的分支**——一個任務合併後再開下一個。
3. 分支名 = 任務內容，不放 Task ID（ID 寫在 PR 內文與 tasks.md）。
4. 判斷用哪個 prefix 的準則：**「如果這個 PR 只能保留一種說法，它是什麼？」** 行為變了 = fix/feat；行為沒變 = refactor/docs/test。一個 PR 裝不下一種說法 = 任務切錯了，回 Planner。

## 4. Coding Agent Rules（硬規則）

1. **不可擴大 scope**：規格沒寫的都不做。發現值得做的事 → 寫進 PR 的「建議後續」段落，不要動手。
2. **不可混合 refactor 與 feature**：move-only PR 裡出現任何行為/文案/樣式值變更即退回。
3. **不可改 unrelated files**：diff 中每個檔案都要能對應到任務的 Files likely affected；lockfile 無故變動 = 退回重做。
4. **不可修改資料格式**，除非任務明確要求：SQLite migration 永遠 append-only（動 `0001_init.sql` = 立即退回）；`commands.ts` DTO、Rust DTO、SQL 欄位三處必須同步動、同 PR 動。
5. **不可偷改 UI 行為**：所有既有 `data-testid` 必須存活；e2e 檔案除非任務授權否則零修改（它們是驗收閘門）。UI 文案為 zh-TW。
6. **不可新增套件**，除非任務明確要求且 maintainer 已核准（masterplan Open Questions 有記錄）。`npm audit fix --force` 永遠禁止。
7. **每次完成必附 git diff summary**（`git diff --stat` + 一句話說明每個檔案為何在清單裡）。
8. 安全紅線（AGENTS.md §10 + 專案邊界）：金鑰不進前端/localStorage/SQLite；不引入 `eval`/`new Function`/動態 import 執行路徑；不放寬 CSP；`core/*` 不得出現 React/DOM/IO import；mock 僅存在於 `import.meta.env.DEV` 守衛之後。
9. 映射單一點原則：metrics→summary 只走 `metricsToBacktestSummary()`；candle 轉換只走 `candleAdapter`；新映射需求 = 開新的單一 helper + 測試，不 inline。
10. Commit 訊息：英文 conventional commit（`feat(ui-port): …` / `fix(db): …`）；PR 內文 zh-TW。

## 5. Reviewer Agent Rules

Reviewer 按固定清單逐項檢查並輸出結論：

| 檢查項 | 具體動作 |
| --- | --- |
| **Scope compliance** | diff 檔案清單 vs 任務 Files likely affected；任何多出來的檔案要求解釋或退回 |
| **Behavior change** | refactor/test/docs PR：確認零行為變更（抽查搬移塊 vs 原始碼、testid 清單、樣式值）；fix/feat PR：確認行為變更 = 規格說的那一個，沒有第二個 |
| **Data compatibility** | migration 檔零改動？DTO 三處同步？既有 DB 讀寫是否受影響（upsert 語義、CASCADE 後果有無說明）？ |
| **UI change** | data-testid 存活（grep diff）；zh-TW 文案；無 scope 外的視覺調整 |
| **API / storage impact** | 新 command 是否在 main.rs 註冊、mockClient 是否同步、錯誤路徑是否回傳 AppError |
| **Build/test evidence** | PR 是否附真實驗證輸出；CI 五個 job 綠；聲稱的手動驗證是否可信（有具體觀察，不是「試過了沒問題」） |
| **Architecture impact** | core 純度未破壞；單一映射點未繞過；安全紅線未觸碰；worker 協定仍是 jobId-only |
| **Merge recommendation** | 三選一：**approve / request-changes（列出必改項）/ escalate to maintainer（涉及規格模糊或 Open Question）** |

Reviewer 的產出格式：逐項 ✅/❌ + 證據（引用檔案:行號）+ 最終建議。發現規格本身有錯 → escalate，不代替 Planner 改規格。

## 6. Standard Coding Agent Prompt Template

> Planner 使用；`{…}` 為填空。各 backlog 任務已附量身版本，此為通用模板。

```text
ROLE: You are a coding agent working on AlphaFactorForge. You execute exactly ONE task, then stop.

READ FIRST (in order):
1. AGENTS.md (collaboration contract — binding)
2. docs/agent-execution-protocol.md §2 (7-step flow) and §4 (hard rules)
3. docs/improvement-backlog.md — task {TASK-ID} ONLY. That task's text is your full specification.

TASK: {TASK-ID} — {one-line title}

GIT:
- Verify clean worktree, fetch origin, branch off latest origin/main as {branch-name}.
- Add one In-Progress line for {TASK-ID} to tasks.md (link the backlog entry).

SCOPE GUARD:
- You may only touch the files listed in the task's "Files likely affected".
- If reality contradicts the spec (file missing, behaviour differs), STOP and report; do not improvise.
- No new dependencies. No schema/migration edits. No e2e edits unless the task explicitly grants it.
- Keep every existing data-testid. UI copy is Traditional Chinese (zh-TW).

IMPLEMENT: follow the task's "Exact implementation plan" step by step.

VALIDATE (paste real output in the PR):
cd alpha-factor-forge && npm run typecheck && npm test && npm run build && npm run e2e
{if Rust files touched: cd src-tauri && cargo check --locked && cargo test --locked}
{task-specific manual checks}

DELIVER:
- Commit: {english conventional-commit title}
- PR body in zh-TW: 摘要 / 改了什麼 / 驗證清單(勾選 acceptance criteria) / 殘餘風險 / git diff --stat
- Then STOP. Do not merge. Do not start another task. List any out-of-scope improvement ideas as
  "建議後續" bullets instead of implementing them.
```

## 7. Standard Reviewer Agent Prompt Template

```text
ROLE: You are the reviewer agent for AlphaFactorForge. You review ONE PR against ONE task spec. You do not write code.

READ FIRST:
1. docs/agent-execution-protocol.md §5 (review checklist — binding)
2. docs/improvement-backlog.md — task {TASK-ID} (the specification this PR claims to implement)
3. The PR diff and description: {PR link / branch}

CHECK, in this order, citing file:line evidence for every ❌:
1. Scope: every changed file maps to the task's "Files likely affected"; no unrelated edits; lockfile unchanged unless authorized.
2. Behaviour: {for refactor/test/docs: prove zero behaviour change — spot-check moved code against original, grep the data-testid list, diff style values} {for fix/feat: the ONE specified behaviour change is present and no other}.
3. Data: no migration edits; TS DTO / Rust DTO / SQL stay in sync; upsert/CASCADE consequences documented.
4. Security & architecture: core/* purity, single-mapping-point rule, no eval/dynamic code, CSP untouched, mock stays behind import.meta.env.DEV, worker protocol jobId-only.
5. Evidence: acceptance criteria all checked with believable validation output; CI jobs green.
6. Spec sanity: if the spec itself seems wrong or ambiguous, say so explicitly.

OUTPUT (zh-TW):
- 逐項清單 ✅/❌ + 證據
- 裁決：approve / request-changes（附必改清單）/ escalate to maintainer（附原因）
- 一句話風險註記（merge 後最可能出事的點）
```

---

## 附錄 A — 任務啟動前的 Planner 自查清單

- [ ] 任務是 S/M 級且一個 session 能完成？（不是 → 先拆）
- [ ] 依賴的前置任務都合併了？（backlog 總覽表的「依賴」欄）
- [ ] 涉及 Open Question 的決策都拿到了？（masterplan §8）
- [ ] 規格的 implementation plan 便宜 agent 能照抄？（動詞 + 檔案 + 驗證點）
- [ ] main 上沒有會撞同檔案的 open PR？

## 附錄 B — 本協定與既有文件的分工

| 文件 | 管什麼 |
| --- | --- |
| `AGENTS.md` | 永久性協作契約（scope、安全、PR 衛生、Codex proxy 特例） |
| `tasks.md` | 任務狀態的唯一事實（Backlog/Next/In Progress/Done） |
| `docs/improvement-backlog.md` | 任務的**規格**（做什麼、怎麼做、怎麼驗收） |
| `docs/agent-execution-protocol.md`（本文件） | 任務的**流程**（誰、什麼順序、什麼格式交付） |
| `handoffs/` | 跨 session 的決策/審查記錄（append-only） |
| `docs/project-audit-masterplan.md` | 為什麼是這些任務（背景與裁決依據） |
