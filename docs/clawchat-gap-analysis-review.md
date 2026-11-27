# ClawChat × AgentChat 差距分析（v1.0）——技术复核报告

**复核基线**：`H:` = clawchat-plugin-hermes-agent `ac00c73`（✅ HEAD 一致）；`I:` = clawchat-plugin-install-cli `871a619`（✅ HEAD 一致）；`A:` = agent-mesh 工作区。
**方法**：解析全文 354 条唯一 `H:`/`I:` 引用，做“文件存在 + 行号越界”机器校验，再逐条抽读源码核对语义；`A:` 引用全部打开代码核验。

## 证据抽查（表：引用 | 核对结果）

> 合格：文件、行号、语义三者都对得上。✅ 表示一致；⚠️ 表示行号与语义部分对不上，或者挂错了引用。

| 引用 | 核对结果 |
|---|---|
| `H:clawchat_gateway/adapter.py:3991-4091`（`_route_inbound_now` 只做 muted→command→mention→coalesce） | ✅ 函数体完全吻合：3997 查 muted → 4008 群命令 → 4057 mention → 4076 @ 立即 flush → 4079 enqueue |
| `H:clawchat_gateway/adapter.py:4642-4672`（flush 时二次查设置） | ✅ `_dispatch_group_batch_now` docstring 明写“re-check mute / reply mode at FLUSH time” |
| `H:clawchat_gateway/adapter.py:7078-7162`（注入 owner + 本群 + **本批发言者**） | ✅ 7121 遍历 `_group_messages_for_metadata()`（7225 起 = 本批 messages），排除 owner/自己/system |
| `H:clawchat_gateway/clawchat_memory.py:80-90`（三域 owner/user/group） | ✅ 无 agent-self 域 |
| `H:clawchat_gateway/rolling_summary.py:118-141`（不可变 .md + 0600 + 原子写） | ✅ `publish_summary`：`atomic_write_text(..., mode=0o600)` |
| `H:clawchat_gateway/history_maintenance.py:1400-1413`（state CAS 只存引用） | ✅ `self.cas(key, state, summary_ref=ref, ...)` |
| `H:clawchat_gateway/history_budget.py:115-152`（24k 块 / 超大消息中间省略） | ✅ |
| `I:docs/agent-protocol.md:330-397`（21001/21003/21007 + 审批卡） | ✅ 2.8 节结构一致 |
| `H:clawchat_gateway/turn_grants.py:96-185`（turn 快照 + 重叠交集） | ✅ `register_turn` → `intersect_turn_grants` |
| `H:clawchat_gateway/tool_grants.py:199-241`（exact>capability>all；失败 fail-closed） | ✅ docstring + 241 行 `except: return fallback()` |
| `H:clawchat_gateway/api_client.py:581-590`（只解析四项，无 turn_limit） | ✅ 581-590 恰为 5 字段构造 |
| `I:docs/agent-protocol.md:405-413` / `207` / `407`（turn_limit 服务端） | ✅ |
| `H:clawchat_gateway/adapter.py:9824-9825`（默认静态整段） | ✅ `_should_use_static_mode` → `return True` |
| `H:clawchat_gateway/greeting.py:16-31` + `adapter.py:3441-3504`（激活问候用 SQLite claim） | ✅ 3500 行 `claim_pending_activation_bootstrap` |
| `I:packages/core/src/installers/hermes.ts:236-292`（危险项 --force 也不覆盖） | ✅ |
| `H:clawchat_gateway/host_tools_guard.py:1485-1552`（ask 逐次审批） | ✅ `_install_grant_approval_gate`，docstring 明写“once-only ask” |
| `A:agentchat/src-tauri/src/lib.rs:227-279` / `:588`（未覆盖引擎返回 None ⇒ 名片显示“凭证 ✓”） | ✅ 277 `_ => None`；588 `credential_ok = credential_guidance(...).is_none()`；App.tsx:564 据此显示“凭证 ✓” |
| `A:agentchat/src/App.tsx:243-247` / `403`（静态绿点）/ `393-447`（`answered` 推断即“已读”） | ✅ 447 行 `{answered ? "已读" : "已送达"}` |
| `A:agentchat/src/App.tsx:468-470`（IME 无 `isComposing` 保护） | ✅ |
| `A:agentchat/src-tauri/src/lib.rs:662-746`（运行中 run 不落库） | ✅ 全文只在 741 行调一次 `complete_turn`，spawn 前无 running 插入 |
| `A:agentchat/src-tauri/src/db.rs:748-807`（消息+turn 同事务） | ✅ |
| `A:docs/design-agentchat.md:350-359`（M0~M2b 打勾） | ✅ |
| `A:run_worker.sh:87-159`（8 分支） | ✅ 159 行 die 里列全 8 个 worker |
| `I:docs/agent-protocol.md:593-620`、`:622-657`、`:661-718`（L82/L83/L89/L92/L489/L497） | ⚠️ **三处不可解析**：该文件仅 593 行。协议本身没问题（`msg-<ULID>` 见 6.§gotcha1，thinking 见 2.5，流式见 2.6、§7），但行号挂错，需改 |
| `H:clawchat_gateway/tool_grants.py:47`（L179 owner 降级提示） | ⚠️ 47 行是 `NON_OWNER_GRANTS_HINT_A`；实际断言在 48 行常量 `OWNER_DOWNGRADED_MESSAGE` |
| `H:clawchat_gateway/memory_scope.py:34-38`（L358 失败关闭） | ⚠️ 那几行描述“无法放置的会话什么都不读”，但真正的执行点在 82-134 `can_read` |
| `H:clawchat_gateway/tool_capabilities.py:167-220`（L181 跨会话路由默认拒绝） | ⚠️ 函数在 165 行；真正“默认拒绝”的判定是 202-213 的 `return False`，整体范围能覆盖，但起始行有偏移 |
| `H:plugin.yaml:15-70`；L69「**55 个 `clawchat_*` 工具**」 | ⚠️ 行号正确、工具数 55 正确，但其中只有 **53** 个以 `clawchat_` 开头（`chat_history_search/read` 不是）。应改为“55 个工具（53 个 clawchat_* + 2 个 chat_history_*）” |
| 全文 354 条 `H:`/`I:` 引用（机器扫描） | ✅ 351 条文件存在且行号有效；仅上列 3 条越界 |

## 遗漏模块

以下逐项对照 `clawchat_gateway/`（80 个模块）、`docs/`、`skills/`、`plugin.yaml`、`livewares/`、`packages/` 人工排查，标出的都是**在差距矩阵和 §6 里没有落位**的能力面：

**P0 级遗漏**

1. **出站依赖“本 turn 的工具证据”（`turn_attachments.py`，498 行）**。模块 docstring 明确写着：只有 host hook 记录的调用与结果是凭据，`MEDIA:` 散文**不是**制品协议，也无法从 latest-session 反推。文档 L276 把它简化成“`MEDIA:<绝对路径>` 或 `clawchat_send_file`”，恰好漏掉了“只允许本回合真实产出”这条反伪造约束。我方 8 个 CLI 都是纯文本信封，群聊里一旦允许发文件，任何模型幻觉写的路径都会直接外发。
2. **媒体边界的网络安全（`docs/media-boundaries.md` + `media_runtime.py` 896 行）**。文档只在 L182/L281 用 `non-owner-boundaries.md:307-386` 一句带过。实际规则是：仅 HTTPS、逐跳 DNS 解析后校验公网单播、锁定 IP 再连、限制重定向次数、`no_proxy` 走直连校验。我方 §6.2 的“远程附件仅公网单播”没有出处，实现时极易做成裸 `reqwest::get`（SSRF/重定向绕过）。
3. **提示注入面清单（`docs/reference/prompt-injection.md`）**。群描述、群成员便签、被引前情、记忆块全部是**不可信内容进入 system 角色**。我方 M3a 恰好要新增这张注入面（群描述 + 未读前情 + mention 前情），矩阵里完全没提“注入内容标为 untrusted”。同文件 1123 行还有 `UNTRUSTED_HISTORY` 标注，文档也没引用。

**P1 级遗漏**

4. **`gate_outcome.py`（21001/21003/21007 → 结构化工具结果）**。L168 只写了 gate 存在，漏了“回执语义”：`approved` 表示服务端**已执行**、不得再调；只有 `approved_retry` 才允许再调一次。这是 M3b 审批卡最关键的状态机。
5. **密集 ACK / 断线重投（`reliable_delivery.py`，Protocol v2 §11.7）**。`DseqAckTracker` 有 0.2s 去抖、30s 重发、10s 恢复超时。文档把「ACK / 负 ACK」简化成 15s trace_id 对齐，丢失了“dense sequence + receipt”这第二种交付保证；而 L351「断线重连」恰恰是我方 lane 崩溃恢复最需要对齐的部分。
6. **`install_identity` / `instance_lock` / `profile_collision` / `profile_sync` / `state_lock`**。L320-322 只列了 4010/STANDBY/profile 冲突的**现象**，没有列模块，也没写「seed 0600 原子硬链接」「无硬链接则不发布身份」这些实现约束。我方单机 App 确实不用复刻，但「重启后同一实例续用同一身份」在 M5「崩溃/断电演练」里有对应需求。
7. **`onboarding.py` / `onboarding_report.py` / `agent_files.py`**。激活时上报 `agent_kind/lane/os/wiki_version`，以及 `greeting.md / friend-greeting.md / onboarding.json` 的**每 Agent 覆盖文件**机制。文档 L318 说“问候是成功信号”，但漏了“问候文案可由 owner 用文件覆盖”——这正是我方「绑定问候」最容易做成硬编码文案的地方。
8. **`install-cli skills/` + `livewares/`**。官方把 skill markdown **单一事实源**放在 install-cli（`skills/<host>/<id>/SKILL.md` + 生成 manifest），插件仓库只是“快照”，且有一致性测试。§1.13 只把技能当“插件内的 6 个内置技能”，漏掉了「跨仓单一源 + 快照同步」的生态结构；N7 的结论（生态面后置）因此少了依据。

**P2 级遗漏（可不改，但应单列一句“已知未覆盖”）**

9. `standalone_send.py` / `terminal_send.py`（脱离管道后发送）、`restart.py`（509 行，重启编排）、`compression_status.py`、`chat_history.py`（624 行，文档只引了 tools.md）。

## 优先级异议

**整体判断**：86 行里，P0 37、P1 35、P2 14，其中 **P0/P1 项有 4 项已完成**，§6「72/4/68」**核算完全正确**。但存在 **1 处定级错误** 和 **3 处矩阵与设计契约的映射错位**：

1. 🔴 **必须改：L548「本地附件/凭据 denylist」定为 P0**。原因是：
   - 我方现在是 CLI 直接执行、没有工具网关，本回合**根本不存在**出站附件路径；
   - 真正会让凭据在本机被读走的是 L545「宿主工具两层防护」（已有 P0）和 L540；
   - 而 P0 的数量直接驱动 §6.1 的 72 项发布门禁，一项没有可交付定义的 P0 会让 M3b 永远无法“P0 清零”。
   - 建议：降为 **P1**，并注明“附 P0 前置：出站媒体必须先有 `turn_attachments` 式的本回合工具证据”。
2. 🟠 **必换：L497「流式 / thinking 模式」定为 P2，且被塞进「M4c EXP/体验补齐」**，但 P2 项按 §6 口径**不占关键路径**。也就是说：M3a 要实现的 `batch_delay_seconds` 攒批、@ 立即 flush、“忙群 hold”、`[failed]` 与无回复令牌的区分，**都依赖“中间状态/thinking 帧”这条能力**——没有它，用户看到的就是最长 30s 的假死。建议拆成两条：**「thinking/中间状态抑制」→ P0/M3a**（对齐 Hermes `minimal|normal|full` 的 runtime-status 抑制）；**「流式分块渲染」保留 P2**。
3. 🟠 **L597/L598 EXP、榜单、race 定为「P2（自有）」，但落在 M4c/M5**。设计契约 M4c=「EXP+动态」、M5=「race+发布」（`A:docs/design-agentchat.md:358-359`）。若真按契约做，EXP 就是 M4c 的**交付物**，不是“P2 可选”。§6.6 L841 又自己说“不应把复刻 EXP 当发布门禁”——两处口径打架。建议在 §3 明写“EXP/榜单/race 移出 v2.0.0 验收范围，契约 §9 的 M4c/M5 定义需同步修订”，否则评审时会出现“契约要求做、差距分析说 P2”的矛盾。
4. 🟡 **L551「codex 群会话第一天起 read-only」定 P0/M3a：方向对，但表述有误导**。现有代码在 `send_message_core` 里对**所有** codex 任务（含 DM）硬写 `sandbox=read-only`（`lib.rs:623-625`），而设计契约 §6.4 要求的是「DM 由 owner 显式选择 read-only/workspace-write，群内强制 read-only」。差距分析把「已默认只读」当成「群只读已就绪」，会掩盖 M3a 真正要做的是**把 DM 的默认值改成显式选择**。建议在 L551 备注“DM 现为无条件 read-only，与契约 §6.4「DM sandbox 显式选择」冲突”。
5. 🟡 **L507「每群项目文件夹」定 P0（按我方设计）**：既 `❓` 无官方证据，又把它算进 72 项 P0 发布门禁，等于用“未验证的产品假设”卡死发布。建议降 P1，或至少在 §6.1 单列“P0 中含 1 项自有假设”。
6. 🟡 **反向错配（被低估的 P1）**：L562「metadata 结构化块」、L571「idle reset」、L572「摘要失败退避」都是 P1，但它们决定 M4b 的**正确性**（重入、幂等、失败不卡死）；而 L569「不可变摘要文件 + CAS」这种存储选型只是 P1。这不是致命错误，但**“正确性 P1 / 选型 P1”混在同一档**，M4b 排期会自然先做存储、后做退避。

## 路线意见

**与设计契约的 roadmap 一致性：基本一致**

- M3a/M3b/M4a/M4b/M4c/M5 的**边界**与契约 §9（`A:docs/design-agentchat.md:354-359`）逐条对应：M3a=建群+三闸门+解析链+codex 只读；M3b=grants 快照+能力矩阵+网关 spike；M4a=chain_react；M4b=三域记忆+摘要；M4c=体验+EXP；M5=发布。
- **偏差 3 处**：(a) 契约 M4c 含「EXP+动态流」，差距分析把 EXP 挪成 P2、动态挪成 M5 后；(b) 契约 M5 含 race，差距分析挪成 P2；(c) 契约 §6.4 的「DM sandbox 显式选择」在差距分析 §6.3 里没有对应行，只在 §5.1 出现。
- 契约 §9 与 §6 的 M3a 验收措辞一致（"mention 未 @ 静默；all 合并窗口攒批；@ 立即派发且批次不重复；codex read-only"），✅。

**「M3a 开工清单」能否直接开工：不能直接开工，需先补 4 处**

1. 🔴 **清单第 1 条“把 §6.2 的 28 项写成可勾选清单”口径错误**：§6.2 实为 **32 行**（4 已完成 + 24 新增 + 4 P1），§6.1 表头写的“M3a 28 / 已完成 4 / 新增 24”是对的，但 §6.8 第 1 条却让人去“§6.2 的 28 项”里勾。开工第一天就会产生验收表数量分歧。应改为“§6.2 32 行，其中 M2 基线回归 4 项、新建 28 项”。
2. 🔴 **清单第 2 条“修 running run 落库 + lease”在 `recover_scan` 里是无效工作，除非同时改 `lane_dispatcher`**：`db.rs:920-935` 每次 tick 都把**所有** non-terminal turn 捞出来、无条件把 running run 标成 `interrupted` 并 rerun——不与 `lease_until` 判断，也不与 `active` 登记表对照。现在“运行中 run 不落库”恰好掩盖了这个问题；一旦按第 2 条落库，**正在真正执行的 turn 会被每个 tick（300ms）标成 interrupted 并重复派发**。清单里必须写明：恢复扫描要先判 `lease_until > now` 且不在 `active` 表内，才能标 interrupted。
3. 🟠 **清单第 3 条 schema v2 缺 3 个 §6.2 依赖的列**：`runs.grants` 已是 v1 就有（L122），但 §6.2“时间戳/迟到提示”要的 `sent_at` 与 §4.1「时间戳分割线」要的**到达时间**要两列；§6.2“flush 二次查设置”要的 `settings_version` 在清单里有、但 M4b 的 `recent_window` 没有；另 `turns.batch_id` 与「多 @ 同 batch」的 `trigger_msg_ids` 共享语义需定清楚（契约 §6.2 L275：多 @ 共享同一 `trigger_msg_ids`）。建议清单第 3 条补一句“`turns.trigger_msg_ids` 语义扩展为 batch 共享”。
4. 🟠 **清单第 8 条“mention 最小闭环”与第 4 条 Gate 顺序矛盾**：官方顺序是 muted → **command** → mention（`adapter.py:3991-4091`）；我方清单第 9 条才做“群设置面板”，而 `reply_mode` 字段默认是 `'all'`（`db.rs:96`）。在 UI 没有 mention 开关之前，所有群消息都会走 `all` 通路，M3a 验收里的「mention 未 @ 静默」在开工期**无法复现**。建议把“群设置面板（至少 reply_mode/muted）”从第 9 条**前移到第 6 条之前**。
5. 🟡 **清单缺“未覆盖引擎的伪绿修复”的明确验收**：第 10 条只说“4 个未探测 engine 改‘未探测’”，但 `doctor()` 的 `auth{}` 只探测 3 个（`lib.rs:195-199`），§4.2 要求的「四态」需要新字段。建议在第 10 条加验收点：“`persona_detail.credential_ok` 新增三态（ok/unknown/missing），UI 不再出现单独的 ✓”。

## 结论

**FIX** —— 证据质量整体很高（354 条引用的行号 **99.2% 可解析**，抽查的 30+ 处核心机制引用语义全部吻合；§0 对“官方源码可证实 / 不可证实”的切分是本报告最值得保留的部分；§6 的 72/4/68 计数经机器复算无误）；但**存在 1 处会把 P0 验收卡死的定级错误、3 处矩阵与契约的映射错位、以及 M3a 开工清单里 1 处会导致重复派发/重复副作用的顺序缺陷**，不能直接作为 M3+ 输入。

**必须修改项清单**

1. 修正 3 处越界引用：`I:docs/agent-protocol.md:593-620 / 622-657 / 661-718` → 改用 2.5、2.6、§6 gotcha1、§7 的正确行号。
2. 修正 2 处小错：`tool_grants.py:47`→`:48`；`plugin.yaml:15-70` 的说法改为“55 个工具（53 个 `clawchat_*` + 2 个 `chat_history_*`）”。
3. **L548「本地附件/凭据 denylist」P0 → P1**，并挂 P0 前置“出站媒体须有本回合工具证据”，从 72 项门禁中移出（总数改为 71/36/35/14）。
4. **L497 拆分**：thinking/中间状态抑制 → **P0/M3a**；流式分块渲染 → 保留 P2。M3a 新增项 +1。
5. **统一 EXP/榜单/race 的口径**：要么改契约 §9 的 M4c/M5，要么在 §3 明写“移出 v2.0.0 验收”，两者不能并存。
6. **§6.8 第 1 条**：把“§6.2 的 28 项”改为“§6.2 32 行 = 4 回归 + 28 新建”。
7. **§6.8 第 2 条**：补“`lane_dispatcher` 消费 `recover_scan` 时必须先判 `lease_until`/`active`，否则落库后会 300ms 一次重复派发”。
8. **§6.8 第 8/9 条调序**：群设置面板（muted/reply_mode 两项）前移到 mention 闭环之前。
9. **补 3 个 P0 级遗漏模块**到 §1：`turn_attachments`（出站证据）、`media-boundaries`（SSRF/公网单播细则）、`prompt-injection`（群描述/前情/记忆的 untrusted 标注）。
10. **L551 补注**：DM 现为无条件 codex read-only，与契约 §6.4「DM sandbox 显式选择」冲突，M3a 需同步改。
11. （建议）L507 降 P1 或单列“P0 中含 1 项自有未验证假设”。

REVIEW-DONE
