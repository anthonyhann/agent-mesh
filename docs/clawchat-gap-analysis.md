# ClawChat × AgentChat 差距分析与落地路线

> 版本：v1.1（2026-11-27）
> 对标对象：ClawChat 官方 Hermes 宿主插件 `main@ac00c73` + 官方安装 CLI `871a619`
> 我方基线：AgentChat 设计契约 v2.4（M0~M2b 已实施）
> 结论先行：**M0~M2b 是真实可运行资产，M3a 是分水岭。真正的差距不在“有没有群聊”，而在群聊闸门、turn 级权限、记忆/摘要、安装与失败引导、以及诚实的状态表达。**

---

## 0. 证据口径与边界

### 0.1 证据别名

为避免长路径污染表格，本文用三个别名：

| 别名 | 绝对路径 | 说明 |
|---|---|---|
| `H:` | `/www/github/clawchat-plugin-hermes-agent` | ClawChat 官方 Hermes 宿主插件，`main@ac00c73` |
| `I:` | `/www/github/clawchat-plugin-install-cli` | ClawChat 官方安装 CLI，`871a619` |
| `A:` | `/Users/hanqiang/www/github/agent-mesh` | 我们的设计契约、内核与 AgentChat 实现 |

引用格式为 `文件:行号` 或 `文件:起-止行`。所有 ClawChat 功能断言只使用 `H:` / `I:` 证据；`A:` 只用于描述我方设计或现状。计数口径：§3 矩阵 90 行（P0 39 / P1 37 / P2 14）；M3a 32 行（4 项回归 + 28 项新建）；v2.0.0 发布门禁 74 项（76 项 P0/P1 扣除 2 项自有未验证假设）。

### 0.2 官方源码覆盖边界（必须先说清）

本次提供的两个官方仓库是**Agent 侧通道实现 + 安装器**，不是 ClawChat 桌面 App、移动端或服务端的完整源码。因此必须区分三类事实：

| 能力面 | 能否由本次官方源码直接证实 | 说明与证据 |
|---|---|---|
| Agent 侧消息、lane、群闸门、@、权限、记忆、摘要、技能、Liveware、CLI、安装 | **能** | `H:clawchat_gateway/*`、`H:plugin.yaml:15-70`、`I:packages/*`、`I:docs/agent-protocol.md` |
| ClawChat 服务端协议行为（turn_limit、permission gate、typing/presence/read） | **部分能** | `I:docs/agent-protocol.md` 是从内部契约导出的子集；`I:docs/agent-protocol.md:3-13` |
| ClawChat 桌面 App UI、机器频道面板、on-device orchestration | **不能** | 官方技能明确说它是“另一个机制”，由桌面 App 决定，未在本仓库实现：`H:skills/clawchat-orchestration/SKILL.md:21-26` |
| 群项目文件夹、房间权限档 `project/full/readonly`、目标解析 `last/sequential/指定` | **不能** | 在 `H:` / `I:` 中未找到实现；这些来自设计文档更早的产品调研，应标为“待补 App 源码验证”，不能当官方事实 |
| EXP / 榜单 | **不能，且大概率不是 ClawChat** | 设计文档自己已澄清：`clawchats.app` 是同名比特币社交 DApp，钱包/上链/EXP 不是本对标对象：`A:docs/design-agentchat.md:47` |
| race（竞速） | **不能** | 仅出现在我方设计：`A:docs/design-agentchat.md:322-324`；`H:` / `I:` 无对应事件、工具或状态机 |
| 群聊“接话 chain”作为独立引擎 | **不能** | 官方没有集中式 chain 调度；只有 `reply_mode=all` + 模型自主判断 + 群描述/行为提示：`I:docs/agent-protocol.md:466-470`、`H:skills/clawchat-orchestration/SKILL.md:114-150` |

> 因此，本文把“官方源码可证实的功能”与“设计文档中的产品假设”严格分开。后者在差距矩阵里会标为 **证据缺口**，而不是“ClawChat 已有”。

### 0.3 复核方法

1. 先以 `plugin.yaml`、协议文档、工具参考建立能力地图，再逐项回到实现文件核对行号。
2. 对每个关键机制同时看“入口 → 状态 → 输出 → 失败路径”，避免只看 happy path。
3. 对我方实现，按设计契约 M0~M2b 的验收项逐条对照代码，不以设计文档的打勾代替代码事实。
4. 对设计文档 §0.2 的 C1~C6 做“保留 / 修正 / 补全”，并在 §1.23 单列，避免复述旧调研。

### 0.4 八个结论

1. **AgentChat 的骨架是对的**：消息+turn 同事务、房间 lane、完成事务、崩溃恢复、同引擎建会话锁、8 引擎私聊，这些不是 PPT，是已落地代码：`A:agentchat/src-tauri/src/db.rs:751-902`、`A:agentchat/src-tauri/src/lib.rs:662-839`。
2. **ClawChat 的群聊不是“一个大脑统一派单”**：每个 Agent 各自连接、各自拿群设置、各自做闸门；`all` 档把每条消息都投给模型，由模型决定说不说：`I:docs/agent-protocol.md:405-413`、`I:docs/agent-protocol.md:466-470`。
3. **设计文档 C2 的“目标解析链”不是官方机制**：官方 `_route_inbound_now` 只做 mute → command → mention → coalescer，没有 `default/last/owner` 目标解析：`H:clawchat_gateway/adapter.py:3991-4091`。这是我方为省 token 做的本地优化，应明确标注“自有差异”。
4. **设计文档 C4 的“记忆三域”映射错了**：官方三域是 `owner / user / group`，不是 `owner / agent-self / room`；群回合注入的是 owner + 本群 + **本批发言者**的 user 便签，不是所有成员：`H:clawchat_gateway/clawchat_memory.py:80-90`、`H:clawchat_gateway/adapter.py:7078-7162`。
5. **设计文档 C5 的摘要存储方式与官方不同**：官方是“不可变摘要文件 + SQLite state CAS 引用”，不是把摘要正文写进 DB 一行：`H:clawchat_gateway/rolling_summary.py:118-141`、`H:clawchat_gateway/history_maintenance.py:1407-1411`。
6. **权限/审批是官方核心，不是可选装饰**：server permission gate（21001/21003/21007）+ 每人 `direct/ask/deny` 工具授权 + host tool guard 两层 + turn 快照交集，构成完整闭环：`I:docs/agent-protocol.md:330-397`、`H:clawchat_gateway/tool_grants.py:199-241`、`H:clawchat_gateway/turn_grants.py:96-185`。我方“审批卡 P2”是现实妥协，但不是完整复刻。
7. **EXP/race 不是 ClawChat 复刻项**：EXP 属于另一个同名 DApp，race 无官方证据；它们只能作为 AgentChat 自有差异化，不能写进“ClawChat 功能对齐”验收。
8. **最危险的不是功能少，而是“装得像”**：当前 UI 的在线点是静态的、已读回执是前端推断的：`A:agentchat/src/App.tsx:243-247`、`A:agentchat/src/App.tsx:393-447`。这会在群聊和权限上线后直接伤害信任，应与 M3a 一起修掉。

---

## 1. ClawChat 完整功能/体验清单

### 1.1 产品定位与账号模型

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| Agent 是完整账号，不是一次性 Bot | 有自己的 `user_id`、昵称、头像、bio、行为提示；可与人类、其他 Agent 同处一个社交网络 | `H:prompts/platform.md:1-9`；`I:docs/agent-protocol.md:70-81` |
| 一个 profile 一个 Agent；多 profile = 多 Agent | Hermes 的 `config.yaml`、`.env`、`activations` 行都是单槽；第二个 Agent 要新 profile，或 OpenClaw 的命名 account | `H:docs/activation.md:93-111`、`H:docs/activation.md:150-165`；`H:docs/install.md:291-332` |
| Channel 与 Host 职责分离 | 通道负责激活、token、WS、入站过滤、群合并/闸门、prompt 渲染、分块、媒体上传；宿主只回答一个 turn | `I:docs/agent-protocol.md:15-23` |
| 55 个工具面（53 个 `clawchat_*` + 2 个 `chat_history_*`） | 账号/好友/会话/动态/记忆/元数据/云编排/Liveware/Widget 等十大类 | `H:plugin.yaml:15-70`；`H:docs/reference/tools.md:1-6`、`H:docs/reference/tools.md:8-56` |
| 身份元数据是“显示/背景”，不是授权 | 昵称、头像、bio、群标题都不构成权限证明；消息级 sender/owner 元数据才是来源 | `H:prompts/platform.md:5`；`H:clawchat_gateway/adapter.py:391-409` |
| Agent 行为（behavior）是 owner 配置的长期人设 | 每回合注入；群描述可覆盖默认回复规则，但不能覆盖结构化 @、行为、平台/隐私规则 | `H:clawchat_gateway/adapter.py:401-405` |
| 一个 Agent 可被 owner 编排其他 Agent/群 | 12 条云编排工具：列 Agent、读权限图、改 behavior、建群、加/移成员、改群 prompt/成员发言设置、签连接码 | `H:skills/clawchat-orchestration/SKILL.md:41-85`；`H:clawchat_gateway/tools.py:1688-1815` |
| 云端编排 ≠ 本机编排 | 云端编排由服务端决定；本机机器频道由桌面 App 决定，二者开关独立 | `H:skills/clawchat-orchestration/SKILL.md:17-26` |

### 1.2 消息模型与投递语义

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| Protocol v2 信封 | `version:"2"`、`event`、`trace_id`、`emitted_at`、`chat_id`、`chat_type`、`payload`；`payload` 键必须存在 | `H:clawchat_gateway/protocol.py:151-173`；`I:docs/agent-protocol.md:150-164` |
| 物化消息 / 回复 | `message.send` / `message.reply`；回复带 `reply_to_msg_id` 与 `reply_preview` | `H:docs/client-integration.md:463-535`、`H:docs/client-integration.md:563-591` |
| 富片段 | `text / mention / image / file / audio / video`；音频是“语音消息”，时长毫秒 | `H:clawchat_gateway/inbound.py:327-350`；`I:docs/agent-protocol.md:232-242` |
| 消息模式 | `normal` 是对话输入；`thinking` 是工具进度/推理/状态，Agent 收到应丢弃 | `H:clawchat_gateway/protocol.py:19-36`；`H:clawchat_gateway/adapter.py:3795-3805`；`I:docs/agent-protocol.md:212-261` |
| 客户端消息 ID 与去重 | `msg-` + 26 位 Crockford Base32 ULID；服务端按 `(recipient, message_id)` 去重；重试必须复用同一 ID | `H:clawchat_gateway/protocol.py:43-73`；`I:docs/agent-protocol.md:276-277`、`I:docs/agent-protocol.md:557-559` |
| ACK / 负 ACK | `message.ack` 按 `trace_id` 关联，15s 超时；`message.error` 带 `retryable/retry_after_ms` | `I:docs/agent-protocol.md:196-210`；`H:docs/client-integration.md:526-534` |
| 入站过滤链 | 先分支非消息帧；再丢 system（permission_result 例外）、self-echo、非 normal mode、不可渲染消息；再合并 mention | `H:clawchat_gateway/adapter.py:3720-3898`；`I:docs/agent-protocol.md:244-258` |
| 入站幂等先落库 | `_claim_message_once` 在业务/LLM 处理前先 claim；claim 失败 fail-closed，不派发 | `H:clawchat_gateway/adapter.py:3832-3867` |
| 自回声防护 | `sender.id == own user_id` 直接丢弃，避免宿主把自己出站 chunk 当新输入导致 interrupt 级联 | `H:clawchat_gateway/adapter.py:3577`、`H:clawchat_gateway/adapter.py:3726-3735` |
| 消息模式为空的兼容 | `""` 等价于 `normal`；非字符串也按 normal 处理 | `H:clawchat_gateway/protocol.py:27-36` |
| 撤回 | 只收不发；`target_message_id` 才是要删的 id；永久 tombstone 防 replay 复活 | `H:docs/client-integration.md:1113-1174` |
| Reaction | 单 emoji 槽位；可加/撤；fire-and-forget | `H:docs/client-integration.md:1070-1104`；`H:clawchat_gateway/tools.py:990-1000` |
| Read cursor / delivered receipt | 协议存在，但 Hermes 插件不声明、不发送、不消费 | `H:docs/client-integration.md:1105-1111`、`H:docs/client-integration.md:1747-1753` |
| 流式 | 可选；插件默认静态整段发送，避免宿主 chunk 语义 | `H:clawchat_gateway/adapter.py:9824-9825`；`I:docs/agent-protocol.md:225-226`、`I:docs/agent-protocol.md:279` |

### 1.3 每聊天 lane 与并发

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 每个 `chat_id` 一条 lane | 同一聊天严格按到达顺序、一次处理一帧；不同聊天并行；lane 跨重连存活 | `H:clawchat_gateway/connection.py:275-280`、`H:clawchat_gateway/connection.py:503-506`、`H:clawchat_gateway/connection.py:3360-3383` |
| lane 不阻塞 WS 读循环 | 处理消息可能要等 ACK；若在 read loop await 会死锁，所以每个聊天独立 worker | `H:clawchat_gateway/connection.py:3360-3370`；`H:tests/test_inbound_dispatch_lanes.py:148-178` |
| 一帧异常只损失该帧 | lane 捕获单帧异常，后续帧继续；连接不重建 | `H:clawchat_gateway/connection.py:3385-3416` |
| 群 turn 串行 | 群 session 有 guard；合并器在 turn 进行中继续攒下一批，空闲后再 flush | `H:docs/configuration.md:256-262`；`H:clawchat_gateway/group_message_coalescer.py:62-68`、`H:clawchat_gateway/group_message_coalescer.py:177-233` |
| 忙时输入策略 | 宿主可配 `interrupt / queue / steer`；默认建议 queue | `H:docs/configuration.md:204-206`；`H:docs/output-visibility.md:186-187` |
| 命令绕过批处理 | `/stop`、`/new` 等命令直接进宿主，不等合并窗口 | `H:docs/configuration.md:262` |
| 发送队列 | 断线时 FIFO 最多 128，丢最旧；终态关闭取消待发 | `I:docs/agent-protocol.md:210`；`H:clawchat_gateway/connection.py:82` |

### 1.4 房间、成员与群资料

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 私聊 = 直接会话 | `chat_type=direct`；每会话一个 Hermes session；对方昵称作为会话名 | `H:docs/configuration.md:250-254` |
| 群聊 = 群会话 | `chat_type=group`；默认**每个群共用一个 Hermes session**，transcript 保留真实发言者；legacy 可按发言者拆 session | `H:docs/configuration.md:250-252` |
| 每个 Agent 一份群设置 | `muted / reply_mode / batch_delay_seconds / version`；服务端返回完整快照，客户端整表替换 | `H:clawchat_gateway/group_settings.py:45-58`、`H:clawchat_gateway/group_settings.py:106-141`；`H:clawchat_gateway/api_client.py:561-594` |
| 服务端 turn limit | 协议返回 `turn_limit` 与 `turn_limit_effective`；默认 3，`-1` 无限，`0~20` 显式；Hermes 插件当前**没有解析该字段** | `I:docs/agent-protocol.md:207`、`I:docs/agent-protocol.md:407`；`H:clawchat_gateway/api_client.py:581-590` 只解析四项 |
| 群资料 | 群标题、描述、群主 id/昵称/类型、参与者 id 列表；刷新后写入 group 便签 metadata | `H:clawchat_gateway/clawchat_metadata.py:155-176` |
| 群参与者注入 | 每回合把 `participant_ids` 渲染成 `id: name (profile_type, current_agent/agent_owner/group_owner)` | `H:clawchat_gateway/adapter.py:6978-7027` |
| 群描述即房间规则 | 群描述可写目的、社交语境、规则、约束、Agent 参与方式；在“说不说/说多少”上优先于默认回复指引 | `H:clawchat_gateway/adapter.py:403`；`H:clawchat_gateway/adapter.py:425-431` |
| 群主与 Agent owner 分离 | `group_owner_id` 是群创建者；`agent_owner_id` 是这个 Agent 的主人；二者不可混用 | `H:clawchat_gateway/adapter.py:391-395` |
| 加成员 | 目标必须已是 Agent 好友；默认要 owner 审批；返回 `pending/forbidden`，不可重试 | `H:skills/clawchat-core/SKILL.md:227`；`H:clawchat_gateway/tools.py:886-903` |
| 退群 | 显式要求才退；Agent 是群主时自动转给最早的人类成员，无人则解散 | `H:skills/clawchat-core/SKILL.md:228`；`H:clawchat_gateway/tools.py:870-883` |
| 建群/改群/成员编排 | 云编排建群、加/移成员、改群描述、改成员发言设置；群主才能管理 | `H:skills/clawchat-orchestration/SKILL.md:51-85`；`H:clawchat_gateway/api_client.py:654-705` |
| 成员加入信号 | `conversation.member_added` 只给群 id，不说是谁；创建群不发 member_added | `I:docs/agent-protocol.md:322`、`I:docs/agent-protocol.md:326` |

### 1.5 群聊三闸门与 coalescer

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| Gate 顺序 | 先持久化入站 claim；等群设置 ≤5s；`muted` → group command → `mention` → 合并/flush | `I:docs/agent-protocol.md:438-457`；`H:clawchat_gateway/adapter.py:3991-4091` |
| 禁言优先级最高 | `muted` 连 slash command 一起挡；设置刷新在途时先等刷新，避免旧缓存多回一句 | `H:clawchat_gateway/adapter.py:3997-4007`；`H:clawchat_gateway/adapter.py:1875-1884` |
| 命令先于 mention | 群命令在 `mention` 闸门之前判断，所以 mention-only 群里 `@agent /new` 仍可用 | `H:clawchat_gateway/adapter.py:4008-4056` |
| `reply_mode=mention` | 未被结构化 @（或 @all）就不产生 turn；消息仍进台账/上下文 | `H:clawchat_gateway/adapter.py:4057-4065`；`I:docs/agent-protocol.md:455` |
| `reply_mode=all` | 每条消息都可进 turn；模型按 group description / behavior 决定是否说 | `I:docs/agent-protocol.md:466-470` |
| 合并窗口 | idle = `batch_delay_seconds`（默认 10s），每次新消息重置；max-wait = `max(30s, idle)` | `H:clawchat_gateway/group_message_coalescer.py:75-106`、`H:clawchat_gateway/group_message_coalescer.py:158-175`；`I:docs/agent-protocol.md:415-422` |
| 合并语义 | 以最后一条为宿主 turn；合并 `was_mentioned`、union mention、合并媒体、保留每个发言人 | `H:clawchat_gateway/group_message_coalescer.py:249-293` |
| 合并 transcript | `ClawChat group messages:` + `[message N] 名字:`，多行保留 | `H:clawchat_gateway/group_message_coalescer.py:23-39`；`I:docs/agent-protocol.md:424-434` |
| flush 时二次闸门 | 设置可能在窗口内变化；flush 时重新等设置、重新查 `muted/mention`，不满足就丢弃并记 no_reply | `H:clawchat_gateway/adapter.py:4642-4672` |
| 忙群不 flush | 上一 turn 还在跑就不发下一批，继续攒；空闲后整批发 | `H:clawchat_gateway/group_message_coalescer.py:62-68`、`H:clawchat_gateway/group_message_coalescer.py:199-233` |
| 群设置刷新 | READY、reconnect、`agent.config.changed` 都重拉；加载前群消息等待最多 5s | `H:clawchat_gateway/adapter.py:1746-1748`、`H:clawchat_gateway/adapter.py:1875-1884`；`H:docs/configuration.md:266` |

### 1.6 @ 与定向派发

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 结构化 mention 是唯一权威 | mention fragment + `context.mentions` 合并；两者缺一都可能丢 mention | `H:clawchat_gateway/inbound.py:279-302`；`I:docs/agent-protocol.md:256-258` |
| `@所有人` 哨兵 | `user_id:"all"` 不展开；每个 Agent 自己判断“包括我” | `H:clawchat_gateway/protocol.py:12-17`；`H:clawchat_gateway/inbound.py:297-302`；`I:docs/agent-protocol.md:504-508` |
| `@` 立即 flush | 被 @ 的消息立即 flush，和已有 pending 合并；不重新计时其他批 | `H:clawchat_gateway/adapter.py:4076-4084` |
| mention 路由元数据 | 每帧渲染 `mentions_current_agent`、`mentioned_users`、`mention_routing`（addressed_to_current_agent / addressed_to_other / no_structured_mentions） | `H:clawchat_gateway/adapter.py:7240-7276` |
| 结构化 @ 覆盖文本 @ | 提示词明确：可见文本里的 `@name`、`you`、`everyone` 不能覆盖 `mention_routing` | `H:clawchat_gateway/adapter.py:425-431` |
| 出站 @ 自动链接 | 文本 `@成员名` 按最长匹配、大小写、邮箱前缀排除、歧义不链接等规则转 mention fragment | `H:clawchat_gateway/mention_autolink.py:1-20`、`H:clawchat_gateway/mention_autolink.py:89-118` |
| 主动 @ 某人 | `clawchat_mention_message` 要求显式 `userId/display`，文本 only；成功后抑制同 turn 普通 follow-up | `H:clawchat_gateway/tools.py:927-965`；`H:clawchat_gateway/mention_message.py:5-8` |
| mention 前情 | @ flush 时把“本 session 还没见过”的群消息按时间正序补在前面；有字符预算与截断提示 | `H:clawchat_gateway/adapter.py:4703-4750`、`H:clawchat_gateway/adapter.py:4892-4950`；`I:docs/agent-protocol.md:459-464` |
| 输入 @名字 | 协议文档说 ClawChat 客户端支持 `named_in_text`（无结构化 mention 时按文本 @ 名字回答）；Hermes 插件当前没有实现该分支 | `I:docs/agent-protocol.md:496-502`；`H:clawchat_gateway/adapter.py:7271-7276` 只有三种 routing |

### 1.7 接话 / chain：官方是“涌现”，不是独立引擎

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 没有集中式目标解析 | 官方路由函数只做 mute/command/mention/coalesce；不选 default/last/owner | `H:clawchat_gateway/adapter.py:3991-4091` |
| 多 Agent 接话靠 `all` + 互相可见 | `all` 把每条群消息都投给模型；Agent 能看到彼此发言，自主决定接不接 | `I:docs/agent-protocol.md:466-470`；`H:docs/configuration.md:250-254` |
| 群描述决定“舞台” | 角色扮演/自由圆桌在群描述里声明“自由接话”；工作群用 `mention` 机械静音 | `H:skills/clawchat-orchestration/SKILL.md:114-150` |
| `batchDelaySeconds` 是节奏 | 角色扮演用延迟控制抢话速度；工作群用长延迟避免刷屏 | `H:skills/clawchat-orchestration/SKILL.md:117`；`I:docs/agent-protocol.md:417-420` |
| 轮数上限在服务端 | `turn_limit` 按“人类发言后第 N 轮”计；连续 Agent 消息算一轮；超限 uplink 被 `agent_turn_limit` 拒绝 | `I:docs/agent-protocol.md:207`、`I:docs/agent-protocol.md:407` |
| 没有客户端 chain 开关 | 官方没有 `chain_react` 字段；工作群/舞台都靠 settings + prompt 组合 | `H:skills/clawchat-orchestration/SKILL.md:114-150`；`H:clawchat_gateway/group_settings.py:45-58` 无该字段 |

### 1.8 权限 grants 与审批

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 服务端 owner-permission gate | 业务 REST 被挡时返回 `21001 pending` / `21003 forbidden` / `21007 not owner turn`；`21001` 在 owner DM 出审批卡，批准后服务端执行 | `I:docs/agent-protocol.md:330-397` |
| 审批窗口 | `allow_once / allow_always / allow_1h / allow_1d` 等；窗口内 `auto_allowed`，读操作有 `approved_retry` 语义 | `I:docs/agent-protocol.md:387-394` |
| 回执去重 | `permission_result` 按 `request_id` 进程级去重；`approved` / `auto_allowed` 表示服务端已执行，不能再调 | `H:clawchat_gateway/permission_result.py:1-42`、`H:clawchat_gateway/permission_result.py:75-137` |
| 每人工具授权 | `direct / ask / deny`；选择器支持 exact tool、capability、all；可带过期时间 | `H:docs/tool-grants.md:5-13`；`H:clawchat_gateway/tool_grants.py:199-241` |
| 解析优先级 | 选择器优先级 exact > capability > all；同一级 deny > ask > direct；多非 owner 发言人取交集 | `H:docs/tool-grants.md:147-178`；`H:clawchat_gateway/tool_grants.py:111-153`、`H:clawchat_gateway/tool_grants.py:253-267` |
| 授权卡 | 只发 owner 激活 DM；10 分钟有效 6 位码；`/clawchat-allow` 确认；拒绝立即生效 | `H:docs/tool-grants.md:196-257`；`H:clawchat_gateway/tool_grants.py:512-554` |
| 敏感后果文案 | `all`/敏感工具会明说“能跑任何命令、读写任何文件、拿凭据、冒充 Agent”；中英繁体三语 | `H:clawchat_gateway/tool_grants.py:460-504` |
| 每次调用审批 | ask 走宿主 approval route；只给 Approve Once / Deny；会话/永久授权视为 deny | `H:docs/tool-grants.md:247-257` |
| 非 owner 主机工具限制 | 两层：toolset 覆盖 + `pre_tool_call`；terminal/files/code/subagent/schedule/browser/memory/session_search/connections/MCP 默认挡 | `H:clawchat_gateway/host_tools_guard.py:1-90`、`H:clawchat_gateway/host_tools_guard.py:170-180` |
| 失败关闭 | 宿主 override 抛错会放开工具，所以插件自己 fail-closed；无法放置的回合直接拒答 | `H:clawchat_gateway/host_tools_guard.py:75-89`；`H:docs/non-owner-boundaries.md:157-161` |
| turn 级快照 | 每回合冻结 speakers、grants、owner 验证、request origin；重叠输入会降级/交集 | `H:clawchat_gateway/turn_grants.py:30-56`、`H:clawchat_gateway/turn_grants.py:96-185` |
| owner 降级提示 | owner 回合中混入他人输入 → “本轮以 guest 权限运行，请 owner 重发” | `H:clawchat_gateway/tool_grants.py:48`（`OWNER_DOWNGRADED_MESSAGE`） |
| owner-only 工具 | 授权管理、云编排、Widget 发送只允许 owner；不能通过 public/all 授权暴露 | `H:clawchat_gateway/tool_capabilities.py:9-30`；`H:docs/non-owner-boundaries.md:163-203` |
| 跨会话路由 | 发起子 Agent、定时任务、Kanban 写操作等视为跨 session，默认拒绝；只允许查看/停止 | `H:clawchat_gateway/tool_capabilities.py:167-220`；`H:docs/non-owner-boundaries.md:416-477` |
| 本地附件边界 | 非 owner 回合默认不允许本地文件；凭据/系统路径 denylist；远程附件限 HTTPS 公网单播（逐跳 DNS 校验、锁 IP、重定向上限） | `H:docs/non-owner-boundaries.md:307-386`；`H:docs/media-boundaries.md:7-20`、`H:docs/media-boundaries.md:73-89`；`H:clawchat_gateway/local_paths.py:18-40`、`H:clawchat_gateway/local_paths.py:81-134` |
| slash 命令权限 | owner 可跑全部；非 owner 只允许 `/new /compress /help /whoami`；群命令走 `group_command_mode` | `H:docs/tool-grants.md:405-417` |
| guest 申请访问 | `clawchat_tool_request` 只接受 capability + 60 字理由；6 小时冷却、24 小时 20 张卡；批准后唤醒原会话 | `H:docs/tool-grants.md:322-377`；`H:clawchat_gateway/tools.py:1855-1936` |

### 1.9 记忆三域

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 三域是 owner / user / group | `owner.md`、`users/<usr_id>.md`、`groups/<cnv_id>.md`；没有“Agent 自己的笔记”这一域 | `H:clawchat_gateway/clawchat_memory.py:80-90` |
| 文件结构 | 正文 + `<!-- clawchat:metadata:start/end -->` 结构化块；metadata 字段按 target 白名单过滤 | `H:clawchat_gateway/clawchat_memory.py:28-66`、`H:clawchat_gateway/clawchat_memory.py:193-205` |
| 写入语义 | `append` / `replace` / 单 span `edit`；append 按段落去重；replace/edit 不碰 metadata | `H:clawchat_gateway/clawchat_memory.py:385-486` |
| 安全路径 | target id 禁止路径分隔/NUL/控制字符；users/groups 目录禁止 symlink；最终 resolve 必须仍在 memory root | `H:clawchat_gateway/clawchat_memory.py:69-143` |
| 写入原子性 | 文件级 `state_file_lock` + tmp/rename 原子写 | `H:clawchat_gateway/clawchat_memory.py:266-267`、`H:clawchat_gateway/clawchat_memory.py:446-463` |
| 读权限 | owner DM 可读全部；他人 DM 只读 owner + 对方；群只读 owner + 本群 + 群成员；无法放置的会话什么都不读 | `H:clawchat_gateway/memory_scope.py:68-134` |
| 追加不限制、替换/编辑受限 | append 任何会话都可追加（不泄露已有内容）；replace/edit 必须先能读 | `H:clawchat_gateway/memory_scope.py:91-95` |
| 回合注入 + 提示注入面（prompt-injection，P0） | 每回合注入 owner.md；群还注入本群 note + **本批发言者**的 user note；群描述、成员便签、被引前情、摘要/记忆块都是不可信内容，进入 system 时必须标为数据而非指令（`UNTRUSTED_HISTORY`） | `H:clawchat_gateway/adapter.py:7078-7162`；`H:docs/reference/prompt-injection.md:9-30`、`H:docs/reference/prompt-injection.md:59-63`；`H:clawchat_gateway/history_transcript.py:14`、`H:clawchat_gateway/history_transcript.py:90`；`H:clawchat_gateway/history_maintenance.py:1120-1121` |
| 注入预算 | `note-cap-user / note-cap-group / note-cap-turn`，长 note 先截断、最长的先让位 | `H:docs/configuration.md:280-287`；`H:clawchat_gateway/note_injection.py:27-78` |
| 记忆卫生 | 平台提示要求“先读后写、只写新事实、约 1000 字后压缩重写、不要把私聊内容带进群” | `H:prompts/platform.md:11-13` |
| 压缩/重置前 sediment | 在 `/new` 或摘要压缩前跑一个合成 turn，只允许读写本会话可见的 note，回复被丢弃 | `H:clawchat_gateway/sediment.py:1-23`、`H:clawchat_gateway/sediment.py:58-88` |
| sediment 目标 | DM：owner 或对方；群：本群 + 最近发言者（最多上限）+ owner（仅群内说过的 owner 事实） | `H:clawchat_gateway/adapter.py:6009-6034` |
| 旧记忆迁移 | 扫全局 MEMORY.md 中的人名事实，发一次 owner 提示，引导迁移到 ClawChat notes | `H:clawchat_gateway/adapter.py:3168-3228` |

### 1.10 滚动摘要与历史预算

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| DM / 群预算 | 默认 25k / 15k token；可 5k~200k；按字符系数估算 | `H:clawchat_gateway/config.py:538-541`；`H:docs/history-budget.md:12-21` |
| 触发 | 真实回答完成后严格超预算；或 idle 到期（DM 24h / 群 6h） | `H:clawchat_gateway/history_maintenance.py:1195-1228`；`H:docs/history-budget.md:20-37` |
| 摘要格式 | 固定四段：`Now / Decided / Open / Index`；事实、决定、待办、可复用索引；不抄密码/长链接 | `H:clawchat_gateway/rolling_summary.py:21-38` |
| 分块 | 每块最多 24k 估算 token；保留整条消息，超大消息中间省略并给 `chat_history_read around=` 提示 | `H:clawchat_gateway/history_budget.py:115-152`；`H:docs/history-budget.md:46-60` |
| 摘要调用 | `timeout=120`，最多 3 块/轮；输出上限默认 4k；截断则翻倍重试到 32k | `H:clawchat_gateway/history_maintenance.py:1381-1413`；`H:docs/history-budget.md:46-56` |
| 不可变发布 | 每次摘要写一个带 header 的 `.md` 文件，0600，原子写；state 用 generation CAS 引用 | `H:clawchat_gateway/rolling_summary.py:118-141`；`H:clawchat_gateway/history_maintenance.py:1407-1411` |
| 重新注入 | 新 session 的第一条真实消息带 `## ClawChat Conversation Summary` + `## Recent messages`；最近窗口默认保留 8 条他方消息 | `H:clawchat_gateway/history_maintenance.py:1104-1123`；`H:docs/history-budget.md:134-141` |
| `/new` 边界 | 用户 `/new` 保留宿主确认；旧 session 先摘要 + 可选 sediment，再重置；边界按 ledger 位置不可变 | `H:docs/history-budget.md:152-173`；`H:clawchat_gateway/history_maintenance.py:1079-1088` |
| 失败退避 | 摘要失败保留旧摘要与 pending，按 1/2/4/8 条真实消息退避；5 分钟维护 watchdog | `H:clawchat_gateway/history_maintenance.py:1337-1347`；`H:docs/history-budget.md:97-108` |
| 预算关闭 | `budget-compaction: off` 回到宿主压缩 + legacy 阈值 note | `H:docs/history-budget.md:23-27` |

### 1.11 Typing / presence / read / delivered

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| typing 是“电平”，不是边沿 | 服务端不保存 typing 状态；客户端持续刷新 `true`，结束/超时/崩溃由接收端本地过期 | `H:docs/client-integration.md:891-907`；`I:docs/agent-protocol.md:281-283` |
| Hermes 插件节流 | 同一聊天连续 `true` 间隔 <1.5s 直接忽略；默认最长连续 900s；turn 结束/优雅关闭发 `false` | `H:clawchat_gateway/adapter.py:249`、`H:clawchat_gateway/adapter.py:1524-1606`、`H:clawchat_gateway/adapter.py:1722-1732` |
| 前端过期 | 参考客户端：人类/未知 6s，已知 Agent 15s；发送前约 4s 续一次 | `I:docs/agent-protocol.md:283`、`I:docs/agent-protocol.md:553` |
| sediment 不点亮 typing | 记忆 sediment 是维护 turn，不给用户看“正在输入” | `H:clawchat_gateway/adapter.py:1525-1527`、`H:docs/configuration.md:315` |
| 死会话不点亮 | `conversation.dissolved` 后清 per-chat 状态，typing 不再上行 | `H:clawchat_gateway/adapter.py:1608-1720`；`H:docs/configuration.md:528` |
| presence 协议存在，但 Hermes 插件不订阅 | 收到 `presence.snapshot/update` 只记日志、丢弃；不发 subscribe | `H:clawchat_gateway/connection.py:3213-3229`；`H:docs/client-integration.md:909-916` |
| read cursor 不实现 | Hermes 不声明 `multi_device`，不发 read，不收回执 | `H:docs/client-integration.md:193`、`H:docs/client-integration.md:1105-1111` |
| delivered receipt 不实现 | 不声明 `delivery_receipt`，不发、不处理；没有回执不等于失败 | `H:docs/client-integration.md:1747-1753`；`I:docs/agent-protocol.md:546` |
| 撤回只收不发 | Agent 不发起撤回；收到 `rcl:` slot id 后按 `target_message_id` 删本地行并写永久 tombstone | `H:docs/client-integration.md:1123-1174` |

### 1.12 EXP / 榜单 / race：必须与 ClawChat 切割

| 功能点 | 本次官方源码能否证实 | 结论 | 证据 |
|---|---|---|---|
| EXP / 经验值 / 等级 | **不能** | 55 个工具里没有 EXP/等级工具；事件目录没有 EXP 事件；设计文档明确 EXP 属于另一个 `clawchats.app` DApp | `H:plugin.yaml:15-70`；`H:docs/client-integration.md:402-447`；`A:docs/design-agentchat.md:47` |
| 榜单 / 贡献榜 | **不能** | 官方没有 leaderboard/ranking/scoreboard 工具或事件；群设置只有 mute/reply/batch/turn limit | `H:plugin.yaml:15-70`；`H:clawchat_gateway/group_settings.py:45-58`；`I:docs/agent-protocol.md:407` |
| race / 竞速 | **不能** | 官方无 race 事件、工具、状态机；仅我方设计 §6.7 保留 | `H:docs/client-integration.md:402-447`；`A:docs/design-agentchat.md:322-324` |

**落地含义**：EXP/榜单/race 应归类为 **AgentChat 自有能力**，不能写进“ClawChat 完整复刻验收”，并**移出 v2.0.0 验收范围**；契约 §9 的 M4c（EXP+动态）与 M5（race+发布）定义需同步修订。如果产品要坚持“积分看贡献”，建议改名“贡献值/被采纳次数”，不要叫 EXP，避免与同名 DApp 混淆。

### 1.13 技能、Liveware、Widget

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 6 个内置技能 | `clawchat-core`、`clawchat-liveware`、`clawchat-liveware-dev`、`clawchat-liveware-sample`、`clawchat-orchestration`、`clawchat-set-greeting`；目录扫描自动发现 | `H:__init__.py:621-689`；`H:docs/skill-updates.md:33-35` |
| 技能热更新 | 服务端 content-free signal 触发；从固定 GitHub tag 拉 manifest；按 sha256 逐字节收敛 | `H:clawchat_gateway/skill_update.py:47-70`、`H:clawchat_gateway/skill_update.py:557-610` |
| 下载校验 | 大小上限 256 KiB；sha256；frontmatter `name/version` 必须匹配；全部校验通过才原子写 | `H:clawchat_gateway/skill_update.py:613-655`、`H:clawchat_gateway/skill_update.py:658-720` |
| owner 同意 | 先给 owner DM 发合并提示；owner 回复“更新/取消”；确认前不写 | `H:clawchat_gateway/adapter.py:2163-2332`；`H:docs/skill-updates.md:45-58` |
| 新技能热注册 | 新 id 立即注册，不需要重启；删除技能用 tombstone，下次加载不再注册 | `H:clawchat_gateway/adapter.py:2301-2312`；`H:docs/skill-updates.md:102-159` |
| Liveware CLI 自动下载 | 固定版本 + 每平台 SHA-256；PATH 优先；缓存校验失败重下；安装锁串行 | `H:clawchat_gateway/liveware_cli.py:30-47`、`H:clawchat_gateway/liveware_cli.py:134-164` |
| Liveware 登录与账号隔离 | Agent 用 token 登录，所有命令必须带 `--account <agent id>`；避免多 Agent 共用登录文件串号 | `H:skills/clawchat-liveware/SKILL.md:27-53`；`H:clawchat_gateway/liveware_cli.py:74-92` |
| Liveware 建应用/绑隧道/注册 | `app create`（public/private/allowlist）→ `tunnel bind 127.0.0.1:port` → 验 `/liveware-status`（bound+relayConnected+live）→ `clawchat_register_app` | `H:skills/clawchat-liveware/SKILL.md:55-128` |
| Liveware 访问控制 | 每个 app 独立 public/private/allowlist；观众身份由隧道注入 `X-User-Id` / `X-Clawchat-User-Id` | `H:skills/clawchat-liveware/SKILL.md:180-227` |
| Liveware Sample 自动开机 | 首次激活后确定性下载/启动/隧道/注册示例 app；失败有 30/60/120/300s 重试；仅默认 profile | `H:docs/liveware-sample.md:1-52`；`H:clawchat_gateway/adapter.py:3322-3411` |
| 首页 Widget | `widget/v1` 文档，最大 16 KiB；owner-only；同一 id 60s 合并，只保留最新快照 | `H:docs/reference/tools.md:472-560`；`H:clawchat_gateway/widget_send.py:232-330` |

### 1.14 社交面：好友、资料、动态

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 好友请求全流程 | 搜索用户、发请求、列出收发请求、接受/拒绝、删除好友 | `H:plugin.yaml:25-31`；`H:docs/reference/tools.md:88-102` |
| 账号资料 | 读/改昵称、头像、bio；头像必须用长期 URL，不能用 14 天过期的聊天附件 URL | `H:docs/reference/tools.md:8-24` |
| 加好友自动问候 | 新非 owner 好友通过后，解析 direct conversation，刷新资料，再跑一次合成问候 turn；按 event_id 去重 | `H:clawchat_gateway/adapter.py:1996-2147`；`H:docs/reference/prompts.md:39-65` |
| 动态（Moments） | 列表、读取（含评论）、创建（文本/图片）、删除、点赞/取消、评论、回复评论、删除评论 | `H:plugin.yaml:32-46`；`H:docs/reference/tools.md:240-253` |
| 动态媒体 | 创建动态可传本地绝对路径图片（插件上传）或长期 URL；失败不创建 | `H:docs/reference/tools.md:240-247` |
| 动态评论信号 | `moment.comment.created/replied` 只给 moment id，不给正文；Agent 发一条无正文 owner note，让模型用 `get_moment` 读取 | `H:clawchat_gateway/adapter.py:1892-1967` |
| 动态可见性 | 只列 friends-only 可见 feed；删除动态/评论受 owner permission gate | `H:docs/reference/tools.md:240-253`；`I:docs/agent-protocol.md:538-539` |

### 1.15 媒体与附件

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 入站媒体 | image/file/audio/video 片段带 URL；文字渲染为图片/链接；下载到 per-agent inbox 后把绝对路径给宿主 | `H:clawchat_gateway/inbound.py:327-350`；`I:docs/agent-protocol.md:522-536` |
| 出站媒体 + turn_attachments 本回合证据（P0） | `MEDIA:<absolute_path>` 或 `clawchat_send_file`；图片内联、其他文件卡片；视频可播放；音频自动成语音气泡。只有宿主 hook 记录的本回合工具调用/结果才能授权附件；`MEDIA:` 散文不是制品协议，latest-session 也无法补授权 | `H:prompts/platform.md:15`；`H:skills/clawchat-core/SKILL.md:230-244`；`H:clawchat_gateway/turn_attachments.py:1-4`、`H:clawchat_gateway/turn_attachments.py:363-366`、`H:clawchat_gateway/turn_attachments.py:443-470` |
| `[[as_document]]` | 强制图片作为文件发送；`send_file(as_document:true)` 对任意文件生效 | `H:clawchat_gateway/adapter.py:336`；`H:skills/clawchat-core/SKILL.md:238` |
| 上传限制 | 协议媒体上限 100 MiB；头像走 `/v1/files/upload-url`；聊天附件约 14 天过期 | `I:docs/agent-protocol.md:512-520`；`H:docs/reference/tools.md:12-16` |
| 入站下载失败 | 跳过失败附件，仍运行 turn；沙箱宿主可能访问不到 URL | `I:docs/agent-protocol.md:522-525` |
| 音频不能“听” | 宿主只能拿到音频文件路径；需本机转写或让用户给文字 | `I:docs/agent-protocol.md:534-536` |
| 本地路径/媒体边界（media-boundaries，P0） | 非 owner 回合默认拒绝本地文件；凭据/系统路径 denylist 在 symlink resolve 后生效；远程媒体仅 HTTPS，逐跳 DNS 解析后全部为公网单播才放行，锁定已校验 IP 再连，最多 5 次重定向，`no_proxy` 直连也复检 | `H:docs/non-owner-boundaries.md:307-386`；`H:docs/media-boundaries.md:7-20`、`H:docs/media-boundaries.md:22-49`、`H:docs/media-boundaries.md:73-89`；`H:clawchat_gateway/media_runtime.py:270-318`、`H:clawchat_gateway/media_runtime.py:559-595` |

### 1.16 历史检索与本地台账

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 当前会话历史搜索 | `chat_history_search(query, from, to, speaker, limit)`；按命中词数、时间排序；默认 10，最多 30 | `H:plugin.yaml:16-17`；`H:docs/reference/tools.md:255-266` |
| 当前会话历史读取 | `chat_history_read(around, from, to, limit)`；默认 20，最多 60；支持 around/range/latest | `H:docs/reference/tools.md:255-266` |
| 只读当前会话 | 本地 SQLite ledger 跨 reset 可查；不允许选 chat/account；不访问服务端 | `H:docs/reference/tools.md:255-266` |
| 脱敏与截断 | URL/凭据脱敏；UTF-16 长度上限；大段内容分页；不返回已撤回/失败/非消息 | `H:docs/reference/tools.md:267-289` |
| 撤回 tombstone | 删除行 + 永久 tombstone；replay 先到、后到都不会复活 | `H:docs/client-integration.md:1159-1170` |
| 本地 ledger 只增不删 | `clawchat_messages` 无 retention；只有 recall 会删；因此 @mention 前情可能读到很久前的消息 | `H:docs/client-integration.md:1155-1158` |

### 1.17 CLI 命令面

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| `hermes clawchat activate CODE` | 激活/重配对；默认后台重启 gateway；支持 `--restart/--no-restart/--new-account/--repair` | `H:clawchat_gateway/cli.py:35-69`、`H:clawchat_gateway/cli.py:91-167`；`H:docs/reference/cli.md:24-34` |
| `hermes clawchat status` | JSON：短 install ID、superseded 状态、winner version、owner notice、锁状态、compression 诊断 | `H:clawchat_gateway/cli.py:99-121`；`H:docs/reference/cli.md:48-59` |
| `hermes clawchat reclaim` | 清 superseded；运行中的 gateway 30s 内重连，否则下次启动连接 | `H:clawchat_gateway/cli.py:122-130`；`H:docs/reference/cli.md:54-59` |
| 会话内 slash | `/clawchat-activate`、`/clawchat-reclaim`、`/clawchat-output minimal|normal|full` | `H:clawchat_gateway/commands.py:81-154`；`H:__init__.py:816-844` |
| `hermes gateway setup` | 插件 setup_fn：提示 activation code + API base URL，不自行重启，由宿主 setup 流程接管 | `H:docs/reference/cli.md:83-94`；`H:clawchat_gateway/setup.py` |
| v0.12 兼容脚本 | `python .../clawchat_cli.py activate CODE`；与 top-level 同一 parser | `H:clawchat_cli.py`；`H:docs/reference/cli.md:36-46` |
| 退出码/失败指引 | 0 成功；1 API/本地前置失败；2 无子命令；失败打印 `next_action`/`hint` | `H:docs/reference/cli.md:110-117`；`H:clawchat_gateway/cli.py:72-88` |

### 1.18 安装、绑定与激活体验

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 安装 CLI 只有 install/update | 目标 `openclaw` 或 `hermes`；支持 `--force`、三个 base URL、`--activate`、`--profile`、`--install-timeout` | `I:packages/cli/src/cli.ts:99-174`；`I:docs/architecture.md:49-110` |
| Hermes 安装流程 | 先拉远程 `plugin.yaml` 检查版本/兼容；`hermes plugins install ... --enable`；更新走 `plugins update` + `enable` | `I:packages/core/src/installers/hermes.ts:558-594`、`I:packages/core/src/installers/hermes.ts:729-787` |
| 安全扫描 | 第一次 install 绝不带 `--force`；扫描拒绝 `SCAN_BLOCKED`；caution 只能 owner 自己确认，安装器不代按 | `I:packages/core/src/installers/hermes.ts:236-350` |
| profile 选择 | `--profile` 映射 `-p` + profile home；混用 `HERMES_HOME` 会双重 profiles，明确警告 | `I:packages/core/src/installers/hermes.ts:36-53`；`I:README.md:64-84` |
| 连接码预检 | `POST /v1/agents/connect/check` 不消耗码；返回 `pairable/status/user_id_status/bound_agent` | `I:docs/agent-protocol.md:64-68`；`H:clawchat_gateway/activate.py:966-981` |
| 一 profile 一 Agent | 已配对再激活会绑定回同一 Agent，不创建第二个；`--new-account` 才新建，`--repair` 才恢复 | `H:clawchat_gateway/activate.py:282-308`、`H:clawchat_gateway/activate.py:982-1025` |
| 已删除 Agent 的陷阱 | 重放旧 `user_id` 会“复活”旧 Agent 及历史；要新 Agent 必须先清 id 或 `--new-account` | `I:install.md:125-145`、`I:install.md:663-674` |
| 激活后重启 | Hermes 插件 reload 不会启动连接；必须重启 gateway；CLI 可能只打印 `plugin_awaiting_restart` | `I:install.md:465-523` |
| 成功信号是问候 | 插件首次连接自动发一条 greeting；用户确认收到才算端到端成功；不能手动代发 | `I:install.md:27-32`、`I:install.md:525-544`；`H:clawchat_gateway/adapter.py:3441-3504` |
| 安装失败分类 | `CLAWCHAT_REPORT:` 固定分类：scan_blocked/host_missing/network/tls/metadata/timeout/... | `I:packages/core/src/progress-report.ts:4-20`、`I:packages/core/src/progress-report.ts:40-82` |
| 安装冲突/4010 | 另一个安装顶替时保留凭据、进入 SUPERSEDED、不重连；owner 收到一次通知；`reclaim` 恢复 | `H:docs/instance-locks-and-install-conflicts.md:109-190`；`H:clawchat_gateway/connection.py:1753-1822` |
| 本机单实例锁 | profile lock + identity lock；竞争进入 STANDBY，不连 WS、不刷新；30s 重试 | `H:docs/instance-locks-and-install-conflicts.md:67-107` |
| profile 冲突检测 | 同一 `base_url` 下两个 profile 用同一 `user_id`，加载时 warning；只读、best-effort | `H:docs/activation.md:127-148` |

### 1.19 UX 细节：问候、输出、回执、文案与节奏

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 激活问候 | 首次连接自动发一条短问候；提示“不要写文件、不要调工具、只问候”；可按 owner 语言追加 `Reply in <Language>` | `H:clawchat_gateway/greeting.py:16-31`、`H:clawchat_gateway/greeting.py:88-126` |
| 新好友问候 | 非 owner 新好友单独一套文案：介绍自己、说明代表 owner、不泄露 owner 隐私 | `H:clawchat_gateway/greeting.py:33-46`；`H:clawchat_gateway/adapter.py:1996-2147` |
| 问候去重 | 激活问候用 SQLite claim；好友问候按 signal event_id 持久去重；重连不重复 | `H:clawchat_gateway/adapter.py:3441-3504`、`H:clawchat_gateway/adapter.py:2061-2080` |
| 输出可见性三档 | `minimal` 只最终答案；`normal` 加自然中间消息；`full` 加推理、工具进度、命令输出、运行时状态 | `H:clawchat_gateway/output_visibility.py:9-52`；`H:docs/output-visibility.md:30-115` |
| 运行时状态抑制 | minimal/normal 下抑制压缩提示、后台自省摘要、stream stalled、gateway shutdown/restart 等噪音 | `H:docs/output-visibility.md:117-151`；`H:clawchat_gateway/adapter.py:7995-8025` |
| CLI 状态行清洗 | `◐ Session automatically reset…`、`◆ Model/Provider/Context` 永远不发给聊天 | `H:clawchat_gateway/hermes_session_status.py:1-33`；`H:clawchat_gateway/adapter.py:9857-9869` |
| 重启中断提示 | 服务重启时处理中的 turn，120s 后给用户一次本地化提示“可能没完成，请重发”；最多一次 | `H:clawchat_gateway/interrupted_turns.py:16-25`、`H:clawchat_gateway/interrupted_turns.py:73-110` |
| 群命令回执 | owner 在群里发命令，结果回到 owner DM，标题 `From group "<title>": <command>`；群里原命令加 ✅，失败也加 | `H:clawchat_gateway/adapter.py:4127-4242` |
| 时间戳与迟到提示 | 每回合有 `sent_at`/`sent_age`；年龄大表示 replay/离线迟到，不要当刚发生 | `H:clawchat_gateway/adapter.py:407`、`H:clawchat_gateway/adapter.py:6935-6954` |
| 无回复令牌 | `<clawchat:no-reply/>` / `NO_REPLY` / `[SILENT]` 等；整条抑制；流式首块要 hold 防止泄漏 | `H:clawchat_gateway/no_reply.py:1-22`、`H:clawchat_gateway/no_reply.py:106-123` |
| 贴纸语法 | 整条消息只有 `[claw:spark.<id>]` 时渲染为 App 贴纸 | `H:prompts/platform.md:19` |
| 语言 | owner locale 已知时追加“用 X 语言回复”；未知时不硬猜英文 | `H:clawchat_gateway/greeting.py:88-105`；`H:clawchat_gateway/owner_language.py` |
| 长消息分块 | 协议建议 4000 字符、markdown-aware 分块；Hermes 插件把 `max_message_length` 设为 0，实际分块由宿主/适配层决定 | `I:docs/agent-protocol.md:278`、`I:docs/agent-protocol.md:549`；`H:__init__.py:500` |
| 群命令回执的边界 | 只有 owner 的命令转 DM；非 owner、turn-starting 命令、approval 回复留在群里 | `H:clawchat_gateway/adapter.py:4127-4148` |

### 1.20 可靠性、状态与安全细节

| 功能点 | 行为/体验 | 源码证据 |
|---|---|---|
| 自回声过滤 | 自己的 user id 出现即丢；否则宿主会把自己的出站 chunk 当新输入 | `H:clawchat_gateway/adapter.py:3577`、`H:clawchat_gateway/adapter.py:3726-3735` |
| 非 normal 模式丢弃 | thinking/progress/status 不作为对话输入 | `H:clawchat_gateway/adapter.py:3795-3805` |
| 死会话 gate | `conversation.dissolved` 后清状态、阻入站、阻 typing；服务端 `chat_not_found/not_member` 也可临时 gate 出站 | `H:clawchat_gateway/adapter.py:1608-1720`；`H:docs/configuration.md:528` |
| 入站速率告警 | 每聊天窗口内新消息过多时 warning，帮助发现 self-echo/interrupt loop | `H:clawchat_gateway/adapter.py:3624-3645` |
| 断线重连 | 指数退避，最大 15s；稳定 5s 后重置计数；auth failure 停止重连 | `I:docs/agent-protocol.md:196-210` |
| Token 刷新 | JWT `exp/iat` 本地解码；`refresh_at = exp - max(30m, min(2h, 25% life)) ±5m`；single-flight；先持久化再换内存 | `H:docs/token-refresh.md:115-140`、`H:docs/token-refresh.md:219-262` |
| 安装被顶替 | 4010 → SUPERSEDED；保留凭据、不重连、不刷新；owner 一次通知；reclaim 可恢复 | `H:docs/instance-locks-and-install-conflicts.md:124-190` |
| 安全扫描不可绕过 | `--force` 不能覆盖 dangerous；caution 必须 owner 明确确认 | `I:install.md:582-601`；`I:packages/core/src/installers/hermes.ts:278-292` |
| 文件权限 | 安装 seed/superseded 状态 0600，目录 0700；锁文件不删除 | `H:docs/instance-locks-and-install-conflicts.md:87-100`、`H:docs/instance-locks-and-install-conflicts.md:134-143` |
| 显示脱敏 | 错误/路径/token/URL 在 CLI 输出前做 redaction | `H:clawchat_gateway/cli.py:72-88`；`H:clawchat_gateway/api_client.py:348-381` |
| 工具结果记录 | 每次 `clawchat_*` 调用记录 tool_name/args/result/error 到 SQLite | `H:clawchat_gateway/plugin_tools.py:49-101` |
| 失败关闭 | grant 读取失败、host catalog 失败、身份无法放置、owner 证明失败，全部拒绝而不是放宽 | `H:clawchat_gateway/tool_grants.py:240-241`；`H:clawchat_gateway/host_tools_guard.py:75-89`；`H:clawchat_gateway/memory_scope.py:34-38` |

### 1.21 C1~C6 复核：哪些对、哪些要改、哪些要补

| 旧结论 | 复核判定 | 需要补的面 |
|---|---|---|
| **C1 回合制消息模型** | **基本正确** | 官方是“每聊天 lane + 宿主只回答一个 turn”，但群消息会先 coalesce 成**一个** turn；不是“每句必一 turn”。还要补：self-echo、非 normal 丢弃、claim-before-await、忙碌队列、命令绕过批处理。证据：`H:clawchat_gateway/connection.py:3360-3383`、`I:docs/agent-protocol.md:15-23`、`H:clawchat_gateway/group_message_coalescer.py:231-293` |
| **C2 群聊三闸门 + 合并窗口** | **方向正确，但缺四个关键事实** | ① 官方没有集中式目标解析链；② `turn_limit` 在服务端，Hermes 插件甚至没解析；③ `muted` 连命令一起挡；④ flush 时会二次查设置；⑤ 还要补 commands bypass、agent reply policy、dead chat、typing 抑制。证据：`H:clawchat_gateway/adapter.py:3991-4091`、`H:clawchat_gateway/adapter.py:4642-4672`、`I:docs/agent-protocol.md:207` |
| **C3 turn 级权限快照** | **部分正确，但对象错了** | 官方的 grants 是“非 owner 对宿主工具的 per-person 授权”，不是“房间 project/full/readonly 权限档”；官方另有 server permission gate 和 approval cards。我们的 codex read-only 是本地 CLI 适配，不是官方机制。证据：`H:clawchat_gateway/tool_grants.py:199-241`、`H:clawchat_gateway/turn_grants.py:96-185`、`I:docs/agent-protocol.md:330-397` |
| **C4 记忆 = Markdown 三域** | **结论对，映射错** | 官方是 owner/user/group；没有 agent-self 域。群注入是 owner + 本群 + 本批发言者，不是所有成员/persona。metadata 标记是 `<!-- clawchat:metadata:start/end -->`，不是 `<!-- agentchat:metadata -->`。证据：`H:clawchat_gateway/clawchat_memory.py:28-90`、`H:clawchat_gateway/adapter.py:7078-7162` |
| **C5 滚动摘要** | **机制对，存储方式错** | 官方摘要正文写不可变 `.md` 文件，DB state 只存引用 + CAS；固定四段 `Now/Decided/Open/Index`；每轮最多 3 块；有 5 分钟维护 watchdog。我方“摘要正文存 `room_summaries`”是简化，不是官方。证据：`H:clawchat_gateway/rolling_summary.py:118-141`、`H:clawchat_gateway/history_maintenance.py:1381-1413`、`H:docs/history-budget.md:46-60` |
| **C6 消息幂等 + typing** | **方向正确** | 还要补：`msg-`+ULID 格式、服务端 `(recipient,message_id)` 去重、claim 在 await 之前、recall tombstone、typing 1.5s/4s/6s/15s 的差异、900s 上限、关闭时清 typing。证据：`H:clawchat_gateway/protocol.py:63-73`、`H:clawchat_gateway/adapter.py:3832-3867`、`H:clawchat_gateway/adapter.py:1524-1606` |

**C1~C6 之外，新增 8 条纠偏：**

1. **N1 群设置是“每 Agent 一份”，不是“每房间一份”。** 所以官方群聊没有“谁该回”的中央解析，只有“每个 Agent 自己要不要回”。证据：`H:clawchat_gateway/group_settings.py:45-58`、`I:docs/agent-protocol.md:405-413`。
2. **N2 服务端 turn limit 与客户端群设置是两层。** 插件 `GroupSettings` 不解析 `turn_limit`，但服务端会在 uplink 拒绝 `agent_turn_limit`。证据：`H:clawchat_gateway/api_client.py:581-590`、`I:docs/agent-protocol.md:207`。
3. **N3 文本 @ 名字存在协议分歧。** 协议文档说客户端支持 `named_in_text`；Hermes 插件提示词明确说文本 @name 不能覆盖结构化 mention。证据：`I:docs/agent-protocol.md:496-502`、`H:clawchat_gateway/adapter.py:425-431`。
4. **N4 presence/read/delivered 是协议能力，不是 Hermes 插件能力。** 插件明确不订阅 presence、不发 read、不声明 delivered。证据：`H:docs/client-integration.md:909-916`、`H:docs/client-integration.md:1105-1111`、`H:docs/client-integration.md:1747-1753`。
5. **N5 输出可见性是一等产品能力。** 三档 preset 会改 display、agent heartbeat、runtime status suppression；不是简单“要不要显示日志”。证据：`H:clawchat_gateway/output_visibility.py:9-52`、`H:docs/output-visibility.md:117-151`。
6. **N6 安装恢复是完整状态机。** 4010、SUPERSEDED、reclaim generation、owner notice、profile/identity lock，缺一不可。证据：`H:docs/instance-locks-and-install-conflicts.md:67-190`。
7. **N7 技能/Liveware/Widget 是生态面，不是核心聊天插件。** 但它们决定“新 Agent 第一天有没有东西可玩”，尤其是自动 Liveware Sample 和首页 Widget。证据：`H:docs/liveware-sample.md:1-52`、`H:docs/reference/tools.md:472-560`。
8. **N8 EXP/race 必须从 ClawChat 复刻清单里剔除。** 见 §1.12；否则会把另一个产品的机制当成交付目标。

---

## 2. AgentChat 现状对照

### 2.1 M0~M2b 已实现清单

> 设计契约的打勾见 `A:docs/design-agentchat.md:350-359`；下表按代码事实复核。

| 里程碑 | 能力 | 现状 | 代码证据 | 备注 |
|---|---|---|---|---|
| M0 | 双根 + kernel 复制 + `current` 软链 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:83-114` | 资源目录/开发回退都覆盖 |
| M0 | login shell PATH + 用户级 CLI 目录增补 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:118-164` | Dock/launchd 启动可用 |
| M0 | doctor 工具/凭证概览 | ✅ 有（部分） | `A:agentchat/src-tauri/src/lib.rs:176-206` | `auth` 只探测 dsh/codex/claude 三个文件级凭证 |
| M0 | sidecar spawn + 独立进程组 + stdout/stderr 抽干 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:331-416` | `setsid` + `killpg` |
| M0 | watchdog 超时 + 合成 timeout/cancelled | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:418-493` | budget = `timeout+30s`，与设计一致；固定 attempts=1 |
| M0 | 无头 selftest | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:873-977` | 覆盖 chat/migrate/lane/cancel/multi/dmall |
| M1 | 绑定向导 + 8 engine 列表 | ✅ 有 | `A:agentchat/src/App.tsx:257-305`；`A:agentchat/src-tauri/src/db.rs:14-24` | 只检测 PATH，不区分“已安装未登录” |
| M1 | 私聊 get-or-create + 自动建项目目录 | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:494-537` | 非法字符替换为 `-` |
| M1 | 消息/房间/SQLite 全表 | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:87-143` | 见 2.3 的“空表”问题 |
| M1 | 真 dsh 两轮续聊 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:979-1060` | 自检依赖真实 dsh 凭证 |
| M2a-1 | `user_version` + `VACUUM INTO` 备份 + 只留 5 份 | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:146-238` | 备份目录 0700 |
| M2a-1 | v0→v1 迁移、messages 重建、保留 id、sqlite_sequence | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:240-383` | 迁移失败回滚保留备份 |
| M2a-1 | 三类 ID：client_msg_id / turn_id / run_id | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:60-83`、`A:agentchat/src-tauri/src/db.rs:105-124` | `client_msg_id` 用 `msg-<ULID>` |
| M2a-2 | 消息 + turn 同事务、重复 client_msg_id 复用 turn | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:748-807` | `ON CONFLICT DO NOTHING` |
| M2a-2 | 完成事务：run + 气泡 + turn 终态 | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:809-902` | 单 SQLite 事务；run 唯一键 |
| M2a-2 | 崩溃恢复：无结果→interrupted、成功缺气泡→backfill、失败→attempt+1 | 🟡 部分 | `A:agentchat/src-tauri/src/db.rs:915-1025` | **运行中的 run 不落库，见 2.3** |
| M2a-2 | 每房间 lane 串行、跨房间并行 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:788-839` | `inflight` + `recover_scan` 头阻塞 |
| M2a-3 | `send_message` 入队即返回，后台消费 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:524-538`、`A:agentchat/src-tauri/src/lib.rs:788-839` | UI 立即回显用户消息 |
| M2a-3 | typing start/beat/end + 前端 6s 熄灭 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:648-658`、`A:agentchat/src-tauri/src/lib.rs:418-469`；`A:agentchat/src/App.tsx:325-355` | 1.5s 节拍，接近官方 1.5s/4s 节奏 |
| M2a-3 | 停止按钮 + SIGTERM→5s→SIGKILL + cancelled 泡 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:540-557`、`A:agentchat/src-tauri/src/lib.rs:425-445`；`A:agentchat/src/App.tsx:381-385` | 按房间找第一个 active run |
| M2b | 8 engine 全接入 | ✅ 有 | `A:run_worker.sh:87-159`；`A:agentchat/src-tauri/src/db.rs:14-24` | dsh/codex/opencode/hermes/arkcli/qwen/pi/claude |
| M2b | per-engine 首轮建会话锁 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:210-225`、`A:agentchat/src-tauri/src/lib.rs:714-728` | 锁覆盖整个首轮，不是只覆盖 spawn |
| M2b | K3 sandbox 会话不可变 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:683-701`；`A:run_worker.sh:93-103` | codex 新会话 `-s`，resume 不带；不一致直接拒单 |
| M2b | 凭证前置（dsh/codex/claude/arkcli） | 🟡 部分 | `A:agentchat/src-tauri/src/lib.rs:227-279` | 只覆盖 4 个 engine；opencode/hermes/qwen/pi 无前置 |
| M2b | persona 多开 + 自动编号 | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:431-466`；`A:agentchat/src/App.tsx:543-550` | 同 engine 永远新建 |
| M2b | behavior.md 编辑 + 注入 | ✅ 有 | `A:agentchat/src-tauri/src/db.rs:468-477`、`A:agentchat/src-tauri/src/lib.rs:776-783`；`A:agentchat/src/App.tsx:575-585` | 只前置到 brief，没有群 prompt/摘要/记忆 |
| M2b | IM 视觉：三栏、毛玻璃、iMessage 气泡、时间分隔、组尾头像 | ✅ 有 | `A:agentchat/src/styles.css:1-20`、`A:agentchat/src/styles.css:41-263` | Apple HIG 方向；微信交互尚未补 |
| M2b | 权限徽标 + 凭证 chip + 名片 | ✅ 有 | `A:agentchat/src-tauri/src/lib.rs:281-290`、`A:agentchat/src-tauri/src/lib.rs:574-590`；`A:agentchat/src/App.tsx:552-585` | 徽标是“引擎默认能力”，不是 turn 快照 |

### 2.2 M3a 之后：未实现 / 仅建表

| 能力 | 现状 | 证据 | 说明 |
|---|---|---|---|
| 建群 / 加成员 / 退群 | ⬜ 无 | `A:agentchat/src-tauri/src/lib.rs:499-570` 只有 personas/rooms/DM/history/send/stop/add_persona/set_behavior/persona_detail | 没有 `create_group` / `add_member` 命令 |
| 房间设置 UI（reply_mode / muted / batch / turn limit / group prompt） | ⬜ 无 | `A:agentchat/src/App.tsx:77-167` 只有 chat/contacts 两个 tab | schema 字段存在但无入口 |
| 群消息合并（coalescer） | ⬜ 无 | `A:agentchat/src-tauri/src/lib.rs:788-839` 只按房间串行消费 turn | 没有 idle/max-wait 批次 |
| 禁言 / all/mention 闸门 | ⬜ 无 | `A:agentchat/src-tauri/src/db.rs:91-104` 仅有字段 | `send_message` 直接取全部 members |
| 自动轮数上限 | 🧱 仅字段 | `A:agentchat/src-tauri/src/db.rs:98` | 无计数器/清零/超限逻辑 |
| 结构化 @ / @all / mention routing | ⬜ 无 | `A:agentchat/src-tauri/src/db.rs:105-109` 消息表无 mentions 字段 | UI 无 @ picker |
| 目标解析链（default/last/owner） | 🧱 仅字段 | `A:agentchat/src-tauri/src/db.rs:94-97` | `send_message` 不读该字段，直接 fan-out 全部 members |
| 接话 `chain_react` | 🧱 仅字段 | `A:agentchat/src-tauri/src/db.rs:99` | 无 chain 调度 |
| turn 级 grants 快照 | 🧱 仅列 | `A:agentchat/src-tauri/src/db.rs:122` `runs.grants` | 无写入/读取/交集 |
| 审批卡 / approvals | 🧱 仅表 | `A:agentchat/src-tauri/src/db.rs:135-137` | 无 UI/服务/工具网关 |
| 记忆三域 | ⬜ 无 | `A:agentchat/src-tauri/src/db.rs:87-143` 无 memory 表；无 memory 目录 | 没有 owner/contact/group notes |
| 滚动摘要 | 🧱 仅表 | `A:agentchat/src-tauri/src/db.rs:131-134` | 无预算/摘要/注入 |
| EXP / 榜单 | 🧱 仅表 | `A:agentchat/src-tauri/src/db.rs:141-143` | 无 ExpService/写入/UI |
| race | 🧱 仅表 | `A:agentchat/src-tauri/src/db.rs:138-140` `run_nodes.winner` | 无 race 调度/winner 逻辑 |
| 动态流 / Moments | ⬜ 无 | `A:agentchat/src/App.tsx:81-82` 只有 chat/contacts | 设计 M4c 的“动态流”尚未开始 |
| 媒体 / 文件 / 语音 | ⬜ 无 | `A:agentchat/src/App.tsx:463-467` 工具条按钮全部 disabled | 只能纯文本 |
| 聊天记录搜索 | ⬜ 无 | `A:agentchat/src/App.tsx:407` 搜索按钮 disabled；列表搜索只过滤房间名/最后一条 | 没有 `chat_history_search/read` 等价物 |
| Markdown 渲染 | ⬜ 无 | `A:agentchat/src/App.tsx:432` 用 `<pre>` 纯文本 | 代码块/链接/粗体不渲染 |
| 真实在线状态 | ⬜ 无（展示层伪在线） | `A:agentchat/src/App.tsx:243-247`、`A:agentchat/src/App.tsx:403` | 所有 engine 都显示绿点+“在线” |
| 真实已读/送达 | ⬜ 无（展示层推断） | `A:agentchat/src/App.tsx:393-447` | `answered` 为真即显示“已读” |
| 中断恢复提示 | ⬜ 无 | 无对应组件；lane 恢复直接重跑 | 缺少“刚才中断，可能未完成”的系统泡 |
| 激活/绑定问候 | ⬜ 无 | `A:agentchat/src/App.tsx:107-113` 只打开 DM | 绑定成功没有“Agent 已就绪”的首条消息 |
| 输出可见性 preset | ⬜ 无 | `A:agentchat/src/App.tsx` 没有设置页 | 没有 minimal/normal/full |
| CLI 命令面 | ⬜ 无 | 只有 Tauri commands，无 headless CLI | 不能脚本化/CI 化 |
| 托盘 / 通知 / 全局快捷键 / 自动更新 | ⬜ 无 | `A:agentchat/src-tauri/tauri.conf.json:12-26` 只有主窗口 | 没有 tray/notification/updater 插件 |
| CSP / 最小权限 | ⬜ 无（CSP 为 null） | `A:agentchat/src-tauri/tauri.conf.json:23-25`；`A:agentchat/src-tauri/capabilities/default.json:6-9` | 与设计 §8 的“CSP + isolation”不一致 |
| 技能 / Liveware / Widget | ⬜ 无 | 无对应代码 | 属于生态面，后置 |
| 安装冲突 / reclaim / multi-profile | 🚫 不适用 | AgentChat 是单机 App，不走 ClawChat 云身份 | 但“失败引导/恢复”体验可借鉴 |

### 2.3 已实现但与契约不一致 / 需要先修的点

| 问题 | 事实 | 影响 | 建议 |
|---|---|---|---|
| **运行中的 run 不落库** | `execute_turn` 只在结束时调用 `complete_turn`，没有 spawn 前插入 `status=running` 的 run：`A:agentchat/src-tauri/src/lib.rs:662-746` | `recover_scan` 的“running→interrupted”和“success 缺气泡→backfill”分支几乎不可达；worker 完成到 `complete_turn` 之间崩溃会重跑，可能重复副作用 | spawn 前插入 running run；完成事务改为 upsert 终态；恢复时先标 interrupted 再决定是否重跑 |
| **lease_until 没有真正使用** | schema 有 `lease_until`，`recover_scan` 不检查；`mark_turn_running` 只改 status：`A:agentchat/src-tauri/src/db.rs:112-118`、`A:agentchat/src-tauri/src/db.rs:904-913`、`A:agentchat/src-tauri/src/db.rs:920-935` | 多实例/异常退出时无法靠 lease 判断“是谁在跑” | 领 turn 时 CAS 更新 lease；恢复扫描只处理过期 lease |
| **群字段已存在，但调度会无闸门 fan-out** | `send_message` 取 `members` 全部传给 `add_user_message_turn`；lane dispatcher 对每个 `target_personas` 都 rerun：`A:agentchat/src-tauri/src/lib.rs:528-536`、`A:agentchat/src-tauri/src/db.rs:938-1001` | 一旦手工/未来建群，所有成员都会接话，绕过 muted/reply_mode/turn limit | M3a 前把 `send_message` 的目标解析改为“闸门 + 定向”；群未实现前禁止建群 |
| **伪在线/伪已读** | 在线点静态；已读由“后面有非 user 消息”推断：`A:agentchat/src/App.tsx:243-247`、`A:agentchat/src/App.tsx:393-447` | 用户会以为 Agent 在线/已读；实际可能是失败或跳过 | M3a 先改成“已送达/已回复/失败/未知”，不要显示“已读”；在线状态接真实健康检查或删掉 |
| **中文输入法 Enter 误发送** | `onKeyDown` 只判断 `Enter && !shiftKey`，没有 `isComposing`：`A:agentchat/src/App.tsx:468-470` | 中文候选词回车会直接发送半句话 | 加 `e.nativeEvent.isComposing || e.keyCode === 229` 保护 |
| **凭证前置只覆盖 4 个 engine** | dsh/codex/claude/arkcli；opencode/hermes/qwen/pi 无 preflight：`A:agentchat/src-tauri/src/lib.rs:227-279` | 用户点发送才发现 CLI 未登录/不可用 | 8 engine 都加真实探测；doctor 显示“已安装/已登录/可用模型/工作区”四级 |
| **engine 锁覆盖整个首轮** | `_engine_guard` 持有到 `run_task_impl` 返回：`A:agentchat/src-tauri/src/lib.rs:714-739` | 同 engine 新建第二个 persona 要等第一个首轮跑完，长任务体验差 | 建会话短锁 + 完成后登记；或在 UI 显示“排队中，因同引擎建会话” |
| **生产/自检双路径** | `send_message_core` 只在 selftest 调用；生产走 `send_message` + lane dispatcher：`A:agentchat/src-tauri/src/lib.rs:594`、`A:agentchat/src-tauri/src/lib.rs:1009` | 两条路径容易漂移（例如任务卡字段/behavior 注入） | 生产与自检共用同一 `build_turn_task`，删掉重复路径 |
| **CSP 为 null** | `A:agentchat/src-tauri/tauri.conf.json:23-25` | 与设计 §8 的安全承诺不一致；LLM 输出若未来支持 HTML 风险更大 | 设置最小 CSP；capabilities 只保留必要 permission；保持纯文本/受控 Markdown |
| **空表/空字段造成“看起来有”** | `scores`、`room_summaries`、`approvals`、`runs.grants`、`room_members.muted` 等：`A:agentchat/src-tauri/src/db.rs:91-143` | 评审/用户可能误以为功能已做 | 在 UI/文档中明确“未启用”；或用 feature flag 隐藏 |

### 2.4 现状结论

1. **可复用资产**：lane、完成事务、崩溃恢复、8 engine 适配、K3、persona 多开、凭证引导、IM 视觉，这些是 M3a~M4b 的地基，不需要推倒重来。
2. **必须先修的底座**：运行中 run 落库、lease、群 fan-out 闸门、伪在线/伪已读、IME 保护、CSP。这些不修，M3a 的群聊会放大错误。
3. **M3a 不是“加一个建群按钮”**：它要把“每房间 lane”升级成“每房间 turn 队列 + coalescer + 闸门 + 目标解析 + 群上下文渲染”，同时保持现有 DM 闭环不回归。
4. **M3b 的权限快照不能只写 prompt**：codex 的 OS sandbox 是硬边界，其他引擎必须明确“近似”并给出能力矩阵；审批卡要先做工具网关 spike，不通过就诚实降级。

---

## 3. 差距矩阵

**图例**：✅ 有；🟡 部分；⬜ 无；🧱 仅建表/仅字段；❓ 证据缺口；🚫 不适用/不做。
**优先级**：P0 = 完整复刻必须；P1 = 体验对齐；P2 = 可选/生态。
**里程碑**：对齐设计契约 M3a / M3b / M4a / M4b / M4c / M5；“M5 后”表示不阻塞 v2.0.0。本节矩阵共 90 行（P0 39 / P1 37 / P2 14）。

### 3.1 消息、lane 与可靠性

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| Protocol v2 信封 + text/mention/image/file/audio/video 片段 | `H:clawchat_gateway/protocol.py:151-173`；`H:clawchat_gateway/inbound.py:327-350`；`I:docs/agent-protocol.md:232-242` | 🟡 只有纯文本消息；`messages` 无片段模型：`A:agentchat/src-tauri/src/db.rs:105-109` | 没有富片段/媒体/mention 片段，后续 UI 无法渲染文件卡片/语音 | P1 | M4c（文本 M3a 已够） |
| `msg-`+ULID 客户端消息 ID + 服务端/本地幂等 | `H:clawchat_gateway/protocol.py:63-73`；`I:docs/agent-protocol.md:276-277`、`I:docs/agent-protocol.md:557-559` | ✅ `client_msg_id = msg-<ULID>` + `UNIQUE(room_id,client_msg_id)`：`A:agentchat/src-tauri/src/db.rs:60-83`、`A:agentchat/src-tauri/src/db.rs:105-110` | 无 | P0 | 已完成（M2a-2） |
| 每聊天串行 lane、跨聊天并行、lane 跨重连 | `H:clawchat_gateway/connection.py:275-280`、`H:clawchat_gateway/connection.py:3360-3383`；`H:tests/test_inbound_dispatch_lanes.py:148-178` | ✅ 每房间 lane + `recover_scan` + `inflight`：`A:agentchat/src-tauri/src/lib.rs:788-839` | 无；但群 fan-out 会破坏“turn 串行、run 可并行”的粒度 | P0 | 已完成（M2a-2），M3a 回归 |
| 群消息 coalescer（idle / max-wait / 合并 mention / 合并媒体） | `H:clawchat_gateway/group_message_coalescer.py:75-106`、`H:clawchat_gateway/group_message_coalescer.py:249-293`；`I:docs/agent-protocol.md:415-434` | ⬜ 无；每条用户消息直接建 turn：`A:agentchat/src-tauri/src/db.rs:748-807` | 群聊会一句一 turn，烧 token、上下文碎片化 | P0 | M3a |
| 忙群 hold：上一 turn 未完不 flush，继续攒批 | `H:clawchat_gateway/group_message_coalescer.py:62-68`、`H:clawchat_gateway/group_message_coalescer.py:199-233` | 🟡 lane 保证同房间串行，但没有“攒下一批”的队列：`A:agentchat/src-tauri/src/lib.rs:788-839` | 忙时新消息没有合并窗口，容易多轮打断 | P0 | M3a |
| 完成事务：run + 气泡 + turn 终态不可分割 | `H:clawchat_gateway/adapter.py:3832-3867`（先 claim 再处理）；`H:clawchat_gateway/connection.py:3385-3416`（单帧失败不污染连接） | 🟡 `complete_turn` 正确，但运行中 run 不落库：`A:agentchat/src-tauri/src/db.rs:809-902`、`A:agentchat/src-tauri/src/lib.rs:662-746` | 崩溃在 worker 完成与 complete_turn 之间会重跑；recover 的 interrupted/backfill 分支不可达 | P0 | M3a 前置修复 |
| 撤回 tombstone + replay 防复活 | `H:docs/client-integration.md:1113-1174` | ⬜ 无 `messages.recalled` 写入/查询；schema 有列：`A:agentchat/src-tauri/src/db.rs:105-109` | 用户撤回后仍可能被当上下文/记忆 | P1 | M4c |
| Reaction（emoji 槽位、可撤） | `H:docs/client-integration.md:1070-1104`；`H:clawchat_gateway/protocol.py:309-332` | ⬜ 无 | 缺少低成本确认/情绪表达，群聊体验生硬 | P1 | M4c |
| Read cursor / delivered receipt | `H:docs/client-integration.md:1105-1111`、`H:docs/client-integration.md:1747-1753` | 🟡 前端“已读/已送达”是推断，不是回执：`A:agentchat/src/App.tsx:393-447` | 伪回执；用户无法判断 Agent 是否真的收到/处理 | P1（诚实修复 P0） | M3a 先改文案，M4c 接真实状态 |
| thinking / 中间状态抑制（runtime-status） | `H:docs/output-visibility.md:117-151`；`H:clawchat_gateway/adapter.py:7991-8025`；`I:docs/agent-protocol.md:253-254` | ⬜ 只有 typing 节拍，无“思考/调用工具/等待审批”中间态；空输出会落 `[failed]`：`A:agentchat/src-tauri/src/lib.rs:633-645` | coalescer 最长 30s 攒批期间像假死；用户无法区分“在思考/在调用工具/失败” | P0 | M3a |
| 4000 字符分块 + Markdown 渲染 | `I:docs/agent-protocol.md:278`、`I:docs/agent-protocol.md:549` | 🟡 纯文本 `<pre>`：`A:agentchat/src/App.tsx:432`；无分块 | 长答案一坨；代码块/链接不可读 | P1 | M4c |

### 3.2 房间、成员与群闸门

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| 私聊 = 隐式房间，session 续接 | `H:docs/configuration.md:250-254` | ✅ `open_dm` get-or-create + cwd + `room_persona_sessions`：`A:agentchat/src-tauri/src/db.rs:494-537`、`A:agentchat/src-tauri/src/db.rs:680-702` | 无 | P0 | 已完成（M1/M2b） |
| 建群 / 成员选择 / 群名 | `H:skills/clawchat-orchestration/SKILL.md:51-59`；`H:clawchat_gateway/api_client.py:665-678` | ⬜ 无 `create_group/add_member` 命令；只有 schema：`A:agentchat/src-tauri/src/db.rs:91-104` | 没有群入口，M3a 无法开始 | P0 | M3a |
| 群资料（标题/描述/群主/参与者） | `H:clawchat_gateway/clawchat_metadata.py:155-176`；`H:clawchat_gateway/adapter.py:6978-7027` | 🧱 只有 `name/group_prompt/owner_persona_id/members` 字段：`A:agentchat/src-tauri/src/db.rs:91-100` | 无群资料拉取/渲染/群主语义 | P1 | M3a |
| 每群项目文件夹（自动创建/绑定 cwd） | ❓ 官方 `H:` / `I:` 未找到群项目文件夹实现；仅协议说宿主拿到 working directory：`I:docs/agent-protocol.md:20` | 🟡 DM 自动建 `~/AgentChatProjects/<名>`：`A:agentchat/src-tauri/src/db.rs:509-515`；群未实现 | 需要 App 源码验证官方行为；我方可在 M3a 先按设计做 | P1（按我方设计；自有未验证假设，不计入发布门禁） | M3a |
| 禁言 `muted`（即时，命令也挡；flush 再查） | `H:clawchat_gateway/adapter.py:3997-4007`、`H:clawchat_gateway/adapter.py:4655-4663`；`H:clawchat_gateway/group_settings.py:45-58` | 🧱 `room_members.muted` 字段，无读写/无 UI：`A:agentchat/src-tauri/src/db.rs:101-104` | 群员无法被静音；未来会刷屏 | P0 | M3a |
| `reply_mode = all / mention` | `H:clawchat_gateway/adapter.py:4057-4065`、`H:clawchat_gateway/adapter.py:4664-4672`；`I:docs/agent-protocol.md:455` | 🧱 `rooms.reply_mode` / `room_members.reply_mode` 字段，无闸门：`A:agentchat/src-tauri/src/db.rs:96-103` | 群里每条消息都会触发回复 | P0 | M3a |
| `batch_delay_seconds` + max-wait | `H:clawchat_gateway/group_settings.py:42-58`、`H:clawchat_gateway/group_message_coalescer.py:88-95`；`I:docs/agent-protocol.md:417-420` | 🧱 `rooms.batch_delay_s` 默认 10，无调度：`A:agentchat/src-tauri/src/db.rs:97` | 没有合并窗口，无法“攒批一次回” | P0 | M3a |
| flush 时二次查设置 + 设置刷新 gate | `H:clawchat_gateway/adapter.py:4653-4672`；`H:clawchat_gateway/adapter.py:1875-1884` | ⬜ 无设置缓存/刷新/flush 复检 | 改了 muted/mention 后还会按旧状态回一句 | P0 | M3a |
| 自动轮数上限 `turn_limit` / `auto_turn_limit` | 服务端：`I:docs/agent-protocol.md:207`、`I:docs/agent-protocol.md:407`；插件未解析：`H:clawchat_gateway/api_client.py:581-590` | 🧱 `rooms.auto_turn_limit` 默认 3，无计数/清零/超限：`A:agentchat/src-tauri/src/db.rs:98` | 群聊无成本/刷屏刹车 | P0 | M3a |
| 群命令模式 + 命令绕过批处理 | `H:docs/configuration.md:262`；`H:clawchat_gateway/adapter.py:4008-4056`；`H:docs/tool-grants.md:405-417` | ⬜ 无 slash command 面 | `/new /stop /help` 等无法在群里即时生效 | P1 | M4a |
| 对其它 Agent 的回复策略（默认不自动回 Agent；@all 不算） | `H:docs/configuration.md:223-231`；`H:clawchat_gateway/adapter.py:3647-3718` | ⬜ 无 sender profile 类型/策略 | 多 persona 群会互相触发死循环 | P1 | M4a |
| 死会话 / 自回声 / 入站速率保护 | `H:clawchat_gateway/adapter.py:1608-1720`、`H:clawchat_gateway/adapter.py:3624-3645`、`H:clawchat_gateway/adapter.py:3726-3735` | ⬜ 无（本地没有 WS 自回声，但多 persona 仍可能循环） | 群聊循环/死房间会烧 token | P1 | M4a |
| 房间权限档 `project/full/readonly` | ❓ `H:` / `I:` 未找到；官方是 per-person host tool grants，不是房间档 | 🧱 `rooms.permission` 默认 `project`，无快照/无执行：`A:agentchat/src-tauri/src/db.rs:93` | 证据缺口；需要 App 源码验证或明确改为“我方设计” | P1（按我方设计） | M3b |

### 3.3 @、目标解析与接话 chain

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| 结构化 mention + `@all` 哨兵 | `H:clawchat_gateway/inbound.py:279-302`、`H:clawchat_gateway/protocol.py:12-17`；`I:docs/agent-protocol.md:504-508` | ⬜ 无 mention 字段/解析/UI | 群聊无法点名，也无法 @所有人 | P0 | M3a |
| mention routing 元数据（addressed_to_current/other/no_structured） | `H:clawchat_gateway/adapter.py:7240-7276`；`I:docs/agent-protocol.md:481-490` | ⬜ 无 | 模型无法区分“在说我”还是“在说别人” | P0 | M3a |
| @ 立即 flush，且不与 pending 批重复 | `H:clawchat_gateway/adapter.py:4076-4084`；`I:docs/agent-protocol.md:455-457` | ⬜ 无批处理，所以“立即”无意义；未来需防重复 | @ 后要快回，但不能绕过 muted/turn limit | P0 | M3a |
| 未读前情（unseen since last turn）与字符预算 | `H:clawchat_gateway/adapter.py:4703-4750`、`H:clawchat_gateway/adapter.py:4892-4950`；`I:docs/agent-protocol.md:459-464` | ⬜ 无 | mention-only 群会漏上下文；重新 @ 时接不上 | P1 | M3a |
| 文本 `@名字`（named_in_text） | 协议文档支持：`I:docs/agent-protocol.md:496-502`；Hermes 插件明确不把它当结构化 mention：`H:clawchat_gateway/adapter.py:425-431` | ⬜ 无 @ picker/自动链接 | 中文用户习惯打 `@名字`，不是选结构化 mention | P2 | M4a |
| 目标解析链 default/last/owner/指定 | ❓ 官方无集中解析：`H:clawchat_gateway/adapter.py:3991-4091`；这是我方为省 token 的设计：`A:docs/design-agentchat.md:271-275` | 🧱 `default_target` / `default_target_persona_id` 字段，未使用：`A:agentchat/src-tauri/src/db.rs:94-97` | 不做则“全员广播”烧钱；做则偏离官方，需要明确自有差异 | P1 | M3a |
| 多 @ 每人独立 turn | ❓ 官方每个 Agent 各自收到消息、各自判断；无“多 @ 拆 turn”概念 | ⬜ 无 | 我方要保证“被 @ 的每个人都答，且各计一轮” | P1 | M3a |
| `all` 档多 Agent 接话（涌现式） | `I:docs/agent-protocol.md:466-470`；`H:skills/clawchat-orchestration/SKILL.md:114-150` | ⬜ 无 | 没有“圆桌讨论”体验 | P0 | M4a |
| 群描述作为舞台规则 | `H:clawchat_gateway/adapter.py:403`；`H:skills/clawchat-orchestration/SKILL.md:124-150` | 🧱 `rooms.group_prompt` 字段，无注入：`A:agentchat/src-tauri/src/db.rs:98` | 群没有“角色/规则/何时说话” | P0 | M4a |
| `chain_react` 接话轮开关 | ❓ 官方无该字段；我方自有：`A:docs/design-agentchat.md:279-284` | 🧱 `rooms.chain_react` 字段，无调度：`A:agentchat/src-tauri/src/db.rs:99` | 没有多轮接话，也没有成本闸门 | P1（自有） | M4a |
| 接话深度/轮数/成本上限 | 服务端 `turn_limit`：`I:docs/agent-protocol.md:207`；我方 `auto_turn_limit` + chain 设计 | 🧱 字段有，逻辑无 | 群聊可能无限对聊/烧 token | P0 | M4a |
| 团队模板（hub/worker/checker、writer/reviewer、并行研究） | `H:skills/clawchat-orchestration/SKILL.md:106-122` | ⬜ 无 | 用户不会配置多 Agent 协作 | P1 | M4a |

### 3.4 权限、审批与安全

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| 服务端 owner-permission gate（21001/21003/21007 + 审批卡） | `I:docs/agent-protocol.md:330-397` | ⬜ 无；本地 CLI 直接执行 | 没有“执行前拦截” | P0（依赖工具网关） | M3b（spike 前置） |
| 每人工具授权 direct/ask/deny | `H:docs/tool-grants.md:5-13`；`H:clawchat_gateway/tool_grants.py:199-241` | ⬜ 无；`runs.grants` 列空置：`A:agentchat/src-tauri/src/db.rs:119-124` | 没有“谁能用哪些工具”的概念 | P0 | M3b |
| 选择器 exact/capability/all + 过期 | `H:docs/tool-grants.md:101-137`；`H:clawchat_gateway/tool_grants.py:371-419` | ⬜ 无 | 只能粗粒度，无法“只给读文件” | P1 | M3b |
| 授权确认卡 + 6 位码 + 10 分钟有效 | `H:docs/tool-grants.md:196-257`；`H:clawchat_gateway/tool_grants.py:512-554` | ⬜ 无 | 授权动作没有可审计/可撤销的 UX | P1 | M3b |
| 每次调用审批（ask） | `H:docs/tool-grants.md:247-257`；`H:clawchat_gateway/host_tools_guard.py:1485-1552` | ⬜ 无；CLI 进程内执行无法拦截 | 只能靠 OS sandbox / prompt 近似 | P0（受限） | M3b spike |
| turn 级 grants 快照 + 多发言人交集 | `H:clawchat_gateway/turn_grants.py:30-56`、`H:clawchat_gateway/turn_grants.py:96-185`、`H:clawchat_gateway/turn_grants.py:253-267` | 🧱 `runs.grants` 列，无写入/交集 | 权限是“展示徽标”，不是“回合事实” | P0 | M3b |
| 宿主工具两层防护（toolset 覆盖 + pre_tool_call） | `H:clawchat_gateway/host_tools_guard.py:1-90`、`H:clawchat_gateway/host_tools_guard.py:1637-1647` | ⬜ 无；CLI 无 hook | 群内非 owner 可能让 Agent 读写本机 | P0 | M3b |
| owner-only 工具/管理动作 | `H:clawchat_gateway/tool_capabilities.py:9-30`；`H:docs/non-owner-boundaries.md:163-203` | ⬜ 无 | 群员可能改群设置/伪授权 | P1 | M3b |
| 跨 session 路由（子 Agent/定时/Kanban） | `H:clawchat_gateway/tool_capabilities.py:167-220`；`H:docs/non-owner-boundaries.md:416-477` | ⬜ 无 | 群员可通过子 Agent 绕过权限 | P1 | M3b |
| 本地附件/凭据 denylist | `H:docs/non-owner-boundaries.md:307-386`；`H:docs/media-boundaries.md:73-89` | ⬜ 无 | 可能把 `.env`/`auth.json` 发出去 | P1；P0 前置：出站媒体必须先有 `turn_attachments` 式本回合工具证据 | M3b |
| slash 命令权限（非 owner 只读命令） | `H:docs/tool-grants.md:405-471` | ⬜ 无命令面 | 非 owner 可尝试危险命令 | P2 | M4a |
| guest 申请访问 + 批准后唤醒 | `H:docs/tool-grants.md:322-377`；`H:clawchat_gateway/tools.py:1855-1936` | ⬜ 无 | 群员被拒后没有自助路径 | P2 | M5 |
| codex 群会话第一天起 read-only | ❓ 官方无 CLI sandbox 概念；这是我方为了 CLI 安全加的：`A:docs/design-agentchat.md:292-294` | 🟡 DM 现为无条件 codex read-only：`A:agentchat/src-tauri/src/lib.rs:623-625`；群未实现 | 群一旦开放，codex 若不 read-only 就有写盘风险；另：DM 硬写 read-only 与契约 §6.4「DM sandbox 显式选择」冲突，M3a 需同步改为 read-only/workspace-write 显式选择 | P0 | M3a |
| per-engine 权限能力矩阵（硬/近似） | ❓ 官方是 host tool guard，不按 CLI 引擎分；我方自有 | ⬜ 无；只有 `grants_badge` 文案：`A:agentchat/src-tauri/src/lib.rs:281-290` | 用户不知道哪个是硬边界、哪个是提示词 | P0 | M3b |
| 审批/权限审计 UI | `H:docs/tool-grants.md:196-257`；`H:clawchat_gateway/plugin_tools.py:49-101` | 🧱 `approvals` 空表：`A:agentchat/src-tauri/src/db.rs:135-137` | 无法回看谁在何时授权了什么 | P1 | M3b |

### 3.5 记忆、摘要与历史

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| `owner.md` | `H:clawchat_gateway/clawchat_memory.py:80-85` | ⬜ 无 | Agent 不记得 owner 偏好/禁忌 | P0 | M4b |
| `users/<id>.md`（每个人/每个 persona 的便签） | `H:clawchat_gateway/clawchat_memory.py:86-87` | ⬜ 无 | 不记得每个联系人的性格/承诺/边界 | P0 | M4b |
| `groups/<id>.md`（本群规则/决定） | `H:clawchat_gateway/clawchat_memory.py:88-89` | ⬜ 无 | 群规则只靠临时 prompt，重启即丢 | P0 | M4b |
| `<!-- clawchat:metadata:start/end -->` 结构化块 | `H:clawchat_gateway/clawchat_memory.py:28-29`、`H:clawchat_gateway/clawchat_memory.py:193-205` | ⬜ 无 | 无法把“正文”和“资料字段”分开 | P1 | M4b |
| 记忆读范围（owner/user/group 域限） | `H:clawchat_gateway/memory_scope.py:68-134` | ⬜ 无 | 群员可能读到别人的私聊记忆 | P0 | M4b |
| 回合注入（owner + 本群 + 本批发言者） | `H:clawchat_gateway/adapter.py:7078-7162` | ⬜ 无；只注入 behavior + 原文：`A:agentchat/src-tauri/src/lib.rs:776-783` | Agent 没有长期上下文，重复问同样的事 | P0 | M4b |
| append 段落去重 + replace/edit + 原子写 | `H:clawchat_gateway/clawchat_memory.py:385-486` | ⬜ 无 | 记忆会重复膨胀/并发截断 | P1 | M4b |
| 压缩/重置前 sediment | `H:clawchat_gateway/sediment.py:58-88`；`H:clawchat_gateway/adapter.py:6009-6149` | ⬜ 无 | 摘要/新会话前丢失“值得记住的事” | P1 | M4b |
| DM 25k / 群 15k 预算 | `H:clawchat_gateway/config.py:538-541`；`H:docs/history-budget.md:12-21` | 🧱 `room_summaries` 表，无预算：`A:agentchat/src-tauri/src/db.rs:131-134` | 历史无限增长，最终撑爆上下文 | P0 | M4b |
| 四段摘要 Now/Decided/Open/Index | `H:clawchat_gateway/rolling_summary.py:21-38` | ⬜ 无 | 压缩后丢失“决定/待办/索引” | P0 | M4b |
| 不可变摘要文件 + state CAS | `H:clawchat_gateway/rolling_summary.py:118-141`；`H:clawchat_gateway/history_maintenance.py:1407-1411` | 🧱 设计是 `room_summaries.summary_text` 单行：`A:docs/design-agentchat.md:314`；`A:agentchat/src-tauri/src/db.rs:131-134` | 无法回滚/审计摘要版本；与官方机制不同 | P1 | M4b |
| 最近 8 条 + recent window | `H:docs/history-budget.md:18-19`；`H:clawchat_gateway/history_maintenance.py:1115-1123` | ⬜ 无 | 摘要后可能只给摘要，丢了最近原文 | P0 | M4b |
| idle reset（DM 24h / 群 6h） | `H:docs/history-budget.md:20-21`；`H:clawchat_gateway/history_maintenance.py:1220-1226` | ⬜ 无 | 长期不聊的会话不会自动收口 | P1 | M4b |
| 摘要失败退避 + 5 分钟 watchdog | `H:clawchat_gateway/history_maintenance.py:1337-1347`；`H:docs/history-budget.md:97-108` | ⬜ 无 | 摘要失败会卡死/反复重试 | P1 | M4b |
| 当前会话历史搜索/读取 | `H:docs/reference/tools.md:255-289` | ⬜ 无；UI 搜索按钮 disabled：`A:agentchat/src/App.tsx:407` | 用户找不回旧结论/文件路径 | P1 | M4c |
| 重启中断提示 | `H:clawchat_gateway/interrupted_turns.py:16-25`、`H:clawchat_gateway/interrupted_turns.py:73-110` | ⬜ 无 | 崩溃后用户不知道“刚才那条是否处理完” | P1 | M4b |

### 3.6 UX、生态与安装

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| 激活问候（成功信号） | `H:clawchat_gateway/greeting.py:16-31`；`H:clawchat_gateway/adapter.py:3441-3504` | ⬜ 无；绑定只打开 DM：`A:agentchat/src/App.tsx:107-113` | 用户不知道 Agent 是否真的可用 | P1 | M3a |
| 新好友问候 | `H:clawchat_gateway/adapter.py:1996-2147` | 🚫 单机无好友关系 | 不适用；可改为“新建 persona 首次私聊问候” | P2 | M5 后 |
| 输出可见性 minimal/normal/full | `H:clawchat_gateway/output_visibility.py:9-52`；`H:commands.py` 同 | ⬜ 无设置页 | 用户不能选择“只看答案/看过程” | P1 | M4c |
| 群命令结果回 owner DM + ✅ | `H:clawchat_gateway/adapter.py:4127-4242` | ⬜ 无群/无命令 | 群管理动作没有反馈闭环 | P2 | M4a |
| 时间戳 `sent_at` / `sent_age` | `H:clawchat_gateway/adapter.py:407`、`H:clawchat_gateway/adapter.py:6935-6954` | 🟡 本地 `created_at` 有，但没有发送时间/迟到提示：`A:agentchat/src-tauri/src/db.rs:105-109` | 离线补跑时会把旧消息当新消息 | P1 | M3a |
| 无回复令牌（silence） | `H:clawchat_gateway/no_reply.py:1-22`、`H:clawchat_gateway/no_reply.py:106-123` | ⬜ 无；Agent 空输出会落 `[failed]`/占位：`A:agentchat/src-tauri/src/lib.rs:633-645` | 群聊里无法“听见但不说话” | P0 | M3a |
| 贴纸 `[claw:spark.*]` | `H:prompts/platform.md:19` | ⬜ 无 | 缺情绪表达；可后置 | P2 | M5 后 |
| 在线状态 | 协议有 presence，但 Hermes 插件丢弃：`H:docs/client-integration.md:909-916`；`H:clawchat_gateway/connection.py:3213-3229` | 🟡 静态绿点：`A:agentchat/src/App.tsx:243-247`、`A:agentchat/src/App.tsx:403` | 伪在线；应改真实健康检查或删除 | P1（诚实修复 P0） | M3a |
| typing 指示 | `H:clawchat_gateway/adapter.py:1524-1606`；`I:docs/agent-protocol.md:283` | ✅ 1.5s 节拍 + 6s 熄灭：`A:agentchat/src-tauri/src/lib.rs:418-469`；`A:agentchat/src/App.tsx:325-355` | 基本对齐；可补“谁在输入”名字 | P0 | 已完成（M2a-3） |
| 媒体/文件/语音 | `I:docs/agent-protocol.md:512-536`；`H:skills/clawchat-core/SKILL.md:230-244` | ⬜ 无；工具条全 disabled：`A:agentchat/src/App.tsx:463-467` | 不能发截图/文件，AI 工作流断裂 | P1 | M4c |
| 动态 Moments | `H:plugin.yaml:32-46`；`H:docs/reference/tools.md:240-253` | ⬜ 无；只有本地“动态流”设计 | 单机不一定需要社交动态；可做本地活动流 | P2 | M5 后 |
| 好友/搜索/资料 | `H:plugin.yaml:25-31`；`H:docs/reference/tools.md:88-102` | 🚫 单机无社交图；联系人=persona | 不适用；可做 persona 搜索/标签 | P2 | M5 后 |
| 技能热更新 | `H:docs/skill-updates.md:45-58`；`H:clawchat_gateway/skill_update.py:557-723` | ⬜ 无 | CLI 自身有技能，App 不管理 | P2 | M5 后 |
| Liveware / Widget | `H:skills/clawchat-liveware/SKILL.md:55-128`；`H:docs/reference/tools.md:472-560` | ⬜ 无 | 可做“本地小应用/桌面小组件”，但非聊天核心 | P2 | M5 后 |
| CLI 命令面 | `H:docs/reference/cli.md:10-94`；`H:__init__.py:800-844` | ⬜ 无 headless CLI | 不能脚本化/CI/远程派单 | P1 | M4c |
| 安装/绑定流程（预检→安装→激活→重启→问候） | `I:install.md:95-145`、`I:install.md:316-544`；`H:clawchat_gateway/greeting.py:16-31` | 🟡 应用内绑定有 doctor+勾选，但没有成功问候/分阶段反馈：`A:agentchat/src/App.tsx:257-305` | 绑定体验比 ClawChat 粗；用户不知道下一步 | P1 | M3a |
| 安装冲突 4010 / reclaim / multi-profile | `H:docs/instance-locks-and-install-conflicts.md:109-190` | 🚫 单机单实例，无云身份 | 不适用；但可借鉴“失败恢复”UX | P2 | M5 后 |
| EXP / 榜单 | ❓ 非 ClawChat；见 §1.12 | 🧱 `scores` 空表：`A:agentchat/src-tauri/src/db.rs:141-143` | 不能作为复刻项；移出 v2.0.0 验收范围 | P2（自有；非发布门禁） | M5 后（移出 v2.0.0） |
| race | ❓ 非 ClawChat；见 §1.12 | 🧱 `run_nodes.winner` 空表：`A:agentchat/src-tauri/src/db.rs:138-140` | 自有差异化；移出 v2.0.0 验收范围 | P2（自有；非发布门禁） | M5 后（移出 v2.0.0） |

> **EXP/榜单/race 口径**：三者移出 v2.0.0 验收范围，不占 M4c/M5 关键路径；设计契约 §9 的 M4c（EXP+动态）与 M5（race+发布）定义需同步修订。

### 3.7 复核补充矩阵（出站证据、媒体边界、提示注入、流式拆分）

以下 4 行计入 §3 矩阵总数（3 个 P0 补漏 + 1 个 P2 流式拆分；thinking 已在 §3.1 L497 原地升为 P0）。

| 功能点 | ClawChat 证据 | 我们现状 | 差距 | 优先级 | 建议落地里程碑 |
|---|---|---|---|---|---|
| 流式分块渲染（从原「流式 / thinking」拆出） | `H:docs/client-integration.md:661-718`；`H:clawchat_gateway/adapter.py:9824-9825`；`I:docs/agent-protocol.md:225-226`、`I:docs/agent-protocol.md:279` | ⬜ 只有最终气泡；无分块渲染 | 长答案一坨；流式不是 v2.0.0 关键路径 | P2 | M5（移出 v2.0.0 验收） |
| 出站附件本回合工具证据（turn_attachments） | `H:clawchat_gateway/turn_attachments.py:1-4`、`H:clawchat_gateway/turn_attachments.py:363-366`、`H:clawchat_gateway/turn_attachments.py:443-470`；`H:docs/media-boundaries.md:91-107` | ⬜ 8 engine 均为纯文本 task card，无出站附件路径：`A:agentchat/src-tauri/src/lib.rs:616-625`；一旦加媒体，幻觉路径可直发 | 需要 turn 级工具调用/结果账本；`MEDIA:` 散文和 latest-session 都不能作为授权 | P0 | M3a |
| 媒体下载网络安全（media-boundaries / SSRF） | `H:docs/media-boundaries.md:7-20`、`H:docs/media-boundaries.md:22-49`、`H:docs/media-boundaries.md:73-89`；`H:clawchat_gateway/media_runtime.py:270-318`、`H:clawchat_gateway/media_runtime.py:559-595` | ⬜ 无媒体下载；若直接用 `reqwest::get` 会漏掉 DNS/重定向/代理校验 | 仅 HTTPS；逐跳 DNS 后全为公网单播；锁 IP 再连；最多 5 次重定向；`no_proxy` 直连复检 | P0 | M3a |
| 提示注入面 untrusted 标注（prompt-injection） | `H:docs/reference/prompt-injection.md:9-30`、`H:docs/reference/prompt-injection.md:59-63`、`H:docs/reference/prompt-injection.md:238-257`；`H:clawchat_gateway/history_transcript.py:14`、`H:clawchat_gateway/history_transcript.py:90`；`H:clawchat_gateway/history_maintenance.py:1120-1121` | ⬜ 只注入 behavior + 原文：`A:agentchat/src-tauri/src/lib.rs:776-783`；群描述/成员便签/前情/记忆块未标 untrusted | 不可信内容进入 system 角色后会被当指令；M3a 新增的群描述 + 未读前情 + mention 前情必须统一标注 | P0 | M3a |

---

## 4. 中国版优化建议

> 适用对象：中国单机用户；不假设云端账号、好友关系或服务端权限网关。
> 判断标准：能否减少一次中文输入的中断；能否让用户知道“谁在回、为什么没回、数据去了哪”。
> 本节建议均为推演，现状断言仍按 `H:` / `I:` / `A:` 证据标注。

### 4.1 中文 IM 习惯：先补“低摩擦”，再谈“像微信”

微信式体验不是换一套气泡皮肤，而是把高频动作做成零学习成本。建议优先补下面 8 个点；其中引用、撤回、免打扰、未读角标是群聊上线后的信任基础设施。

| 体验点 | 中国用户预期 | 建议（推演） | §3 矩阵依据 | 里程碑 |
|---|---|---|---|---|
| 引用回复 | 长按/右键消息即可“引用”，被引原文显示在气泡上方；群聊里引用谁就回谁，不改变 @ 目标 | 消息模型增加 `reply_to_msg_id` + `reply_preview`，Agent prompt 带被引原文的截断版；M3a 可先做文本引用，M4c 再做结构化引用卡与“跳到原消息” | §3.1「Protocol v2 信封 + 富片段」（L488）；官方 reply 语义见 `H:docs/client-integration.md:563-591` | M3a 文本引用，M4c 结构化 |
| 撤回 | 2 分钟内可撤回；群里显示“你撤回了一条消息”；撤回后 Agent 不应再引用它 | 双向撤回 + 永久 tombstone；撤回时从上下文、摘要候选、记忆中排除；不触发 Agent 回复。官方只收不发（`H:docs/client-integration.md:1113-1174`），我方本地可做双向，但必须保留 tombstone | §3.1「撤回 tombstone + replay 防复活」（L494） | M4c |
| 消息免打扰 | “免打扰”是不弹通知，不是“Agent 不许回”；@我仍要提醒；夜间可整体静音 | 拆成两个概念：`notify_mode`（all/mentions/off + 免打扰时段）与 `room_members.muted`（Agent 回复闸门）；不要把免打扰误做成禁言 | §3.2「禁言 muted」（L508）；§3.1「Read cursor / delivered receipt」（L496） | M3a 禁言，M4c 免打扰 |
| 输入中提示 | 对方头像旁出现“正在输入…”，多人时显示“谁在输入”；长任务要知道是在思考还是调用工具 | 保留现有 1.5s 节拍与 6s 熄灭；群聊补 persona 名；把“思考中/调用工具/等待审批”做成独立状态；摘要、记忆维护、sediment 不点亮 typing | §3.6「typing 指示」（L588） | M3a |
| 时间分割线 | 5 分钟以上显示时间；今天/昨天/周几/日期分层；跨天有粘性日期 | 统一本地时区与“发送时间/到达时间”两个字段；群聊按天分组；历史回放时标“迟到消息”，不要伪装成刚发生 | §3.6「时间戳 `sent_at` / `sent_age`」（L584） | M3a |
| 未读角标 | 会话列表有未读数；@我红色角标；免打扰房间只显示小点；点开即清零 | 持久化 `last_read_msg_id`；按房间聚合未读；@我单独计数；窗口可见且聚焦时再清；不要用“最后一条是 Agent 回复”推断已读 | §3.1「Read cursor / delivered receipt」（L496）；§3.6「在线状态」（L587） | M3a 基础，M4c 真实游标 |
| 中文输入法 | 候选词回车不能发送半句话；Shift+Enter 换行；粘贴长文不自动发送；@ 选择器不抢输入法按键 | 发送前判断 `isComposing` 与 `keyCode === 229`；@ picker 在组合输入期间不拦截 Enter；粘贴超过阈值只提示“作为文件发送？” | §3.6「typing 指示」（L588）；现状缺陷 `A:agentchat/src/App.tsx:468-470` | M3a（发版前必须） |
| 截图/文件/语音 | 截图直接粘贴；文件显示卡片；图片可预览；语音可转文字；工具条不能长期是 disabled | 先做图片/文件，再做语音；本地路径只在用户明确选择时上传；文件卡片显示大小/类型/来源；语音默认本地转写，失败给文字入口 | §3.6「媒体/文件/语音」（L589） | M4c |

**建议的交互取舍**：群聊默认 `mention` 或定向派发，不默认 `all`；中文用户习惯直接打 `@名字`，但结构化 mention 才是权威，文本 @ 只作为兜底候选，不应覆盖结构化路由（§3.3「文本 `@名字`」L526、「mention routing」L523）。

### 4.2 八引擎并存：把“支持 8 个”变成“知道该用哪个”

八引擎不是简单堆适配，而是中国 CLI 生态的现实：DeepSeek Harness、Codex、OpenCode、Hermes、ArkCLI、Qwen、Pi、Claude Code 各有认证、模型、权限和输出形态。ClawChat 官方插件是 Hermes/OpenClaw 单宿主，安装器只覆盖 OpenClaw/Hermes（`I:packages/cli/src/cli.ts:99-174`），且一个 profile 只对应一个 Agent（`H:docs/activation.md:93-111`）；八引擎并存是我方结构性差异，不是负担。

现状是：引擎注册表已有 8 个（`A:agentchat/src-tauri/src/db.rs:14-24`），非交互分支已有 8 条（`A:run_worker.sh:87-159`），但 §3.4 的 per-engine 权限矩阵仍是空白（L552），§3.6 的绑定只有“已检测/未安装”两态（L595）。建议把“引擎选择”做成产品能力，而不是让用户自己猜。

| 用户问题 | 建议（推演） | 依据 | 里程碑 |
|---|---|---|---|
| 第一次该绑哪几个？ | 推荐“3+5”分层：dsh（重型/浏览器）、codex（编码/review）、arkcli（豆包/结构化）作为起步三件套；opencode/hermes/qwen/pi/claude 按需补绑；不要求一次全绑 | §3.6「安装/绑定流程」（L595）；§3.4「per-engine 权限能力矩阵」（L552）；路由规则 `A:AGENTS.md:22` | M3a |
| 装了但没登录怎么办？ | 每引擎四态：已安装、已登录、模型可用、工作区可写；状态卡直接给下一步命令，不用用户去翻文档 | §3.4「per-engine 权限能力矩阵」（L552）；§3.6「安装/绑定流程」（L595） | M3a |
| 这个任务该派给谁？ | 规则路由预设：重型/浏览器→dsh，编码/review→codex，多模型→opencode，轻量→hermes，豆包/JSON Schema→arkcli，长上下文→qwen，过程可观测→pi，Claude 需 API key；UI 显示“为什么推荐”，允许手动覆盖 | §3.3「团队模板」（L533）；`A:AGENTS.md:22`；`A:README.md:42-49` | M4c |
| 不同引擎到底强在哪？ | 做一张能力卡：上下文窗口、工具调用、结构化输出、流式/事件流、token/成本、会话续接、权限硬度；版本变化时重新标注，不写死“永远最强” | `A:README.md:42-49`；§3.6「输出可见性」（L582）；§3.1「thinking/中间状态抑制」（L497）；§3.7「流式分块渲染」 | M4c |
| 这次为什么慢/贵？ | 每引擎记录延迟、tokens、成本、重试次数；Claude 的 `cost_usd`、Qwen 的 usage、Pi 的事件流统一进 turn 卡片 | §3.6「输出可见性」（L582）；§3.1「thinking/中间状态抑制」（L497）；§3.7「流式分块渲染」 | M4c |
| 升级后行为变了？ | doctor 记录 CLI 版本与探测时间；能力矩阵按版本打标；升级后提示“建议重跑 3 个冒烟任务” | §3.6「安装/绑定流程」（L595）；§3.4「per-engine 权限能力矩阵」（L552） | M4c |
| 失败到底怪谁？ | 错误码分层：引擎未安装、未登录、SSO 过期、模型不可用、额度不足、网络、超时、权限拒绝、工作区不存在；每条给一个可执行动作 | §3.4「per-engine 权限能力矩阵」（L552）；§3.6「安装/绑定流程」（L595） | M3a |

### 4.3 凭证与失败引导：国内 SSO/API key 的真实状态

现状不是“统一账号”，而是四种凭证模型：

- **dsh**：只探测 `~/.dsh/openai-gateway/api-key` 文件是否存在（`A:agentchat/src-tauri/src/lib.rs:231-233`）。
- **arkcli**：走 `arkcli auth status` 探测火山 SSO 登录态，60s 缓存；refresh_token 过期会失效（`A:agentchat/src-tauri/src/lib.rs:246-269`）。
- **codex**：只探测 `~/.codex/auth.json`（`A:agentchat/src-tauri/src/lib.rs:234-235`）。
- **claude**：探测 `ANTHROPIC_API_KEY` 环境变量或数据目录 `claude.env`（`A:agentchat/src-tauri/src/lib.rs:237-245`）。

opencode/hermes/qwen/pi 没有 spawn 前探测；更危险的是 `credential_guidance` 对未覆盖引擎返回 `None`，名片因此显示“凭证 ✓”（`A:agentchat/src-tauri/src/lib.rs:277-278`、`:588`）。这不是凭证通过，而是“没检查”。建议把“未探测”作为独立状态，禁止伪绿。

| 场景 | 用户现在看到 | 建议（推演） | 依据 | 里程碑 |
|---|---|---|---|---|
| 火山 SSO 过期 | 预检命中后给“运行 `arkcli auth login volc-sso`”的提示；60s 内不重复探测 | 一键复制命令 + “登录后自动复检”；复检成功直接补发原消息；不要在群里暴露 token/账号细节 | `A:agentchat/src-tauri/src/lib.rs:246-269`；§3.4「per-engine 权限能力矩阵」（L552） | M3a |
| dsh key 文件存在但不可用 | 文件级探测通过就放行，真正失败在 spawn 后 | 增加无副作用健康检查（版本、网关可达、模型列表），失败时明确是 key、网关还是模型问题 | `A:agentchat/src-tauri/src/lib.rs:231-233`；§3.4「per-engine 权限能力矩阵」（L552） | M3a |
| codex 登录态 | 只看 `auth.json` 是否存在 | 显示登录账号/过期时间（能拿到才显示）；拿不到就标“未验证”；续聊失败自动提示重新登录 | `A:agentchat/src-tauri/src/lib.rs:234-235`；§3.6「安装/绑定流程」（L595） | M3a |
| Claude API key | 环境变量或 `claude.env`；未配置时给明确引导 | 优先 OS Keychain；文件方案保持 0600；错误日志脱敏；绝不要求用户把 key 发到聊天里 | `A:agentchat/src-tauri/src/lib.rs:237-245`；`A:run_worker.sh:130-137`；§3.4「本地附件/凭据 denylist」（L548） | M3b |
| qwen/opencode/hermes/pi | 名片可能显示“凭证 ✓”，实际没探测 | 先改成“未探测”，再逐引擎补 status 探测；没有 status 命令的，用版本 + 配置 + 首次 dry-run 组合 | `A:agentchat/src-tauri/src/lib.rs:277-278`、`:588`；§3.4「per-engine 权限能力矩阵」（L552） | M3a |
| 首次发送失败 | 只有“检查名片中的凭证状态，或点此消息上方重发”的通用提示（`A:agentchat/src/App.tsx:434-435`） | 错误分类 + 单按钮：重新登录、切换引擎、复制错误、改 brief、稍后重试；保留用户原文不丢 | §3.6「安装/绑定流程」（L595）；§3.1「完成事务」（L493） | M3a |
| 重试策略 | 执行失败可能重试；`criteria_failed` 不应重试 | 只对 timeout/网络/可重试错误自动重试；凭证/权限/模型不存在不重试；`criteria_failed` 引导改 brief 或转派 | §3.1「完成事务」（L493）；§3.6「无回复令牌」（L585） | M3a |
| SSO 中途失效 | 下一轮直接失败，用户不知道为什么上一轮还行 | 失败时强制刷新凭证状态；同一 turn 内不重复弹卡；恢复后给“已重新登录，是否重发” | `A:agentchat/src-tauri/src/lib.rs:246-275`；§3.4「per-engine 权限能力矩阵」（L552） | M4c |

**国内化细节**：火山 SSO、内网网关、国产模型 API key 的文案要区分“公司 SSO”“个人 API key”“本机 CLI 登录”三种语义。不要把 `auth.json`、`claude.env`、`api-key` 混成一句“请登录”，否则用户不知道该去哪个终端。

### 4.4 隐私本地化：卖“数据不出机”之前，先定义“什么不出机”

AgentChat 自身没有云账号或云同步：SQLite 在 `~/Library/Application Support/AgentChat/agentchat.db`，项目在 `~/AgentChatProjects`，内核复制到 Application Support（`A:agentchat/src-tauri/src/lib.rs:28-31`、`:888`、`:1061-1063`）。这是真实的本地化资产，可以成为卖点。

但八引擎里多数推理请求会离开本机，具体取决于 provider 配置；Pi 可以指向本地 dsh gateway（`A:run_worker.sh:151-157`），qwen/arkcli/claude/codex 则可能走云。因此建议把卖点写成 **“本地优先 + 可验证的出机边界”**，而不是绝对的“数据不出机”。绝对化承诺一旦被用户抓到一次云端请求，信任就没了。

| 承诺 | 可验证做法（推演） | 不能怎么说 | 依据 | 里程碑 |
|---|---|---|---|---|
| 会话与项目默认本地 | DB、项目目录、内核、备份都在本机；提供“导出全部/删除全部”；不做默认遥测 | “所有数据永远不出机” | `A:agentchat/src-tauri/src/lib.rs:28-31`、`:888`、`:1061-1063`；§3.5「记忆读范围」（L563） | M3a |
| 推理出机边界可见 | 每个 persona/引擎显示“本机/内网网关/云端 API”；turn 卡片记录实际 endpoint 类型；用户可一键切换“只用本机引擎” | “AgentChat 不上传，所以模型也不上传” | §3.4「per-engine 权限能力矩阵」（L552）；`A:README.md:42-49` | M3b |
| 附件与凭证不误发 | 文件选择器只给用户明确选择的路径；canonical path + denylist（`.env`、`auth.json`、key、系统目录）；远程附件只允许公网单播 | “本地路径自动可用” | §3.4「本地附件/凭据 denylist」（L548） | M3b |
| 记忆域隔离 | owner/user/group 三域；群回合只注入 owner + 本群 + 本批发言者；群员不能读别人私聊记忆 | “群里所有人都能看到记忆” | §3.5「记忆读范围」（L563）、「回合注入」（L564） | M4b |
| 撤回即退出上下文 | tombstone 后不再进 prompt、摘要、记忆；历史搜索也不返回已撤回内容 | “撤回只是 UI 隐藏” | §3.1「撤回 tombstone」（L494） | M4c |
| 历史搜索本地完成 | 本地 FTS/索引；支持 around/range/latest；脱敏后再展示；不把整段历史发给模型做检索 | “搜索会顺便上传历史” | §3.5「当前会话历史搜索/读取」（L573） | M4c |
| 出机日志可审计 | 每 turn 记录引擎、endpoint 类型、prompt/completion tokens、是否上传附件；用户可导出 | “我们保证没有日志” | §3.4「per-engine 权限能力矩阵」（L552）；§3.6「输出可见性」（L582） | M4c |

### 4.5 中国版取舍：不建议照抄的三件事

1. **不要把“社交网络”当核心**：Moments、好友搜索、榜单在单机场景是 P2，且 EXP/榜单并非 ClawChat 官方能力（§3.6 L590、L591、L597；§1.12）。中国单机用户更在意“我的 Agent 能不能把活干完”，不是“有没有动态圈”。
2. **不要默认 `all` 模式**：多 Agent 群最容易变成互相刷屏和烧 token。默认 mention/定向，`all` 必须显式开启，并配轮数/成本上限（§3.3 L527、L529、L532）。
3. **不要用“假在线/假已读/假凭证”换完成度**：伪状态在单机私聊还能糊弄，进群后会被“为什么它没回/到底看没看”放大（§3.6 L587；§3.1 L496；§3.4 L552）。宁可显示“未知”，也不要显示假的绿点或“凭证 ✓”。

---

## 5. 自有能力迭代建议

> ClawChat 的护城河是云端社交图谱；AgentChat 的护城河是 **本机多 harness + 可编排 + 可追溯**。
> 建议不要把它做成“一个更小的 ClawChat”，而是做成“我的机器上有一支可审计的 AI 团队”。

### 5.1 七项自有能力如何放大

| 自有能力 | 现状证据 | 放大方向（建议） | 里程碑 | 验收指标 |
|---|---|---|---|---|
| **多 harness 引擎矩阵（8 engine）** | 8 engine 注册表与八条非交互分支：`A:agentchat/src-tauri/src/db.rs:14-24`、`A:run_worker.sh:87-159`；能力差异见 `A:README.md:42-49`；ClawChat 官方是 Hermes/OpenClaw 单宿主安装（`I:packages/cli/src/cli.ts:99-174`） | 从“支持 8 个”升级为 **Engine Capability Matrix 2.0**：安装/登录/模型/上下文/工具/流式/token/成本/会话续接/权限硬度十维能力卡；再往前做“任务类型→引擎”规则预设与双引擎复核模板。矩阵必须按 CLI 版本更新，不写死厂商排名 | M3b 真实性矩阵，M4c 路由预设与成本面板，M5 对外文档 | 8/8 引擎首轮成功或给出明确凭证引导；8/8 登录态真实可解释；规则路由建议被用户接受率可统计 |
| **persona 多开（同 engine 多联系人）** | 同 engine 永远新建实例并自动编号：`A:agentchat/src-tauri/src/db.rs:431-466`；名片“再开一个实例”：`A:agentchat/src/App.tsx:543-550`；会话按 `(room_id, persona_id)` 隔离：`A:docs/design-agentchat.md:198` | 把 persona 做成“员工名片”：头像/姓名/专长/模型/工作区/权限/记忆七件套；支持同 engine N 个 persona 在同一个群扮演不同角色；支持“克隆并改差异”“导出/导入 persona 包”。重点不是多开数量，而是 **同 engine 多实例不串台、不互等** | M3a 群成员，M4a 角色模板，M4b 每个 persona 记忆，M5 导入导出 | 同 engine 3 persona 并发私聊不串台；同群角色不共享私聊记忆；克隆后 behavior/模型/工作区差异可审计 |
| **engine 并发锁** | 首轮建会话 per-engine 锁只覆盖 dsh/codex/opencode/hermes/qwen：`A:agentchat/src-tauri/src/lib.rs:218-225`；锁一直持有到 `run_task_impl` 返回：`:714-741` | 拆成 **建会话短锁 + 运行锁**：能提前拿到 session id 的引擎，拿到即释放；只能结束时拿到 id 的引擎，显式排队并显示“因同引擎建会话，前面还有 N 个”。再加 per-engine 并发预算、超时降级、可切换引擎 | M3a 正确性，M4c 队列 UI，M5 自适应调度 | 第二个 persona 首 token 等待时间可解释；锁持有时间 < 建会话所需；无 session 串台；长任务不再阻塞同 engine 其他 persona |
| **凭证前置引导** | 只有 dsh/codex/claude/arkcli 四个有 spawn 前探测：`A:agentchat/src-tauri/src/lib.rs:227-279`；doctor 的 auth 只覆盖 dsh/codex/claude：`:195-199`；未覆盖引擎会显示“凭证 ✓”：`:277-278`、`:588` | 做 **8-engine Readiness Board**：已安装/已登录/模型可用/工作区可写四态；每个失败态给“复制命令/打开终端/重新检测”；SSO 过期自动复检；全程不要求把 key 发进聊天 | M3a 四态与伪绿修复，M4c 自愈与账号切换，M5 首次安装向导 | 首次发送因凭证失败率下降；从安装到首条成功回复的步骤数可测；0 个“未探测却显示 ✓” |
| **Tauri 原生桌面** | 当前单窗口、无 tray/notification/updater，CSP 为 null：`A:agentchat/src-tauri/tauri.conf.json:12-26`；三栏 IM 视觉：`A:agentchat/src/App.tsx:117-166` | 把桌面能力变成“Agent 在场感”：@我/回合完成/失败的原生通知；托盘未读；全局快捷键“快速问一句”；拖拽文件进聊天；`agentchat://` 深链；自动更新；M5 做签名/公证。CSP 与 capabilities 必须先收紧，再谈 Markdown/HTML | M4c 通知/托盘/快捷键/拖拽，M5 签名公证与 updater | 通知点击回到对应 turn；托盘未读数与 DB 一致；冷启动时间；干净 Mac 双击可用；CSP 非 null |
| **dispatcher 管线（run_pipeline / run_dag）** | 串行 `{{prev_output}}`、超长转文件引用：`A:run_pipeline.sh:1-20`、`:52-67`；DAG `depends_on`/`{{nodes.N.output}}`/`max_parallel`/拓扑分层：`A:run_dag.py:1-30`、`:44-70`；调用入口见 `A:AGENTS.md:16-18` | 做“**workflow room**”：群聊不只聊天，还能把一次群任务变成 pipeline/DAG；聊天里显示节点卡、依赖箭头、单节点重试、人工审批门、产物归档；复用 `run_manifest.json` + `flow.html`，不要另起一套编排。App 目前未接内核管线，这是可补的空白，不是已有能力 | M4a 团队模板，M4c 群内 workflow UI，M5 定时/触发与对外 API | 3 节点串行与 3 节点 DAG 可从 App 发起；单节点失败可只重跑该节点；流程成功率/人工介入次数可统计 |
| **session.html 子会话时间线** | `render_session.py` 已支持事件流时间线、工具调用、token/成本、同会话续接卡：`A:render_session.py:1-8`、`:30-88`；README 说明每次派单自动生成：`A:README.md:53-55`；设计上作为内嵌 fallback：`A:docs/design-agentchat.md:207` | 从“结果目录里的 HTML”升级为 **聊天内子会话抽屉**：每条 Agent 回复可展开“它做了什么”；工具调用可折叠、token/成本可见、产物可下载、失败可重放；与 `turn_id`/`run_id` 对齐，支持导出 Markdown/HTML。Pi 的事件流最适合做样板 | M4c 内嵌抽屉，M5 导出/分享/对比 | 长任务可解释率；用户打开时间线比例；排障平均时间；同一 turn 的消息、run、session.html 可互跳 |

### 5.2 组合效应：把“群聊”卖成“可审计的交付”

单项能力容易被抄，组合起来才难：**8 engine × persona × workflow × session timeline**。建议 v2.0 的演示不要停在“三个 Agent 在群里聊天”，而是跑一次真实交付：

1. dsh 做资料调研（重型/浏览器）；
2. codex 做代码实现与 review；
3. pi 用事件流展示思考/工具调用；
4. arkcli 用 JSON Schema 做结构化验收；
5. 群聊里显示每个节点的状态、token、失败重试；
6. 点开每条回复的 `session.html` 子会话，看到完整证据链；
7. 最后导出 `flow.html` + `session.html` + 产物目录。

这条链路对应的是 §3.3 的团队模板缺口（L533）和 §3.4 的跨 session 路由缺口（L547），也是 ClawChat 云端编排无法直接替代的本地能力：**数据、项目、日志、产物都在用户机器上，但每个 Agent 的权限和过程都可见。**

---

## 6. 行动路线

§3 矩阵共 90 行：**P0 39 项、P1 37 项、P2 14 项**。P0/P1 共 76 项；扣除 2 项自有未验证假设（L507 每群项目文件夹、L516 房间权限档）后，**v2.0.0 发布门禁 74 项**（含 4 项 M2 基线回归，实际新增 70 项）。M5 另加 6 条发布检查（不计入 74 项）。P2 不占关键路径；EXP/榜单/race 移出 v2.0.0 验收范围，契约 §9 的 M4c/M5 定义需同步修订。

> 以下里程碑、实现要点与依赖均为**建议**，不表示已实施；现状事实仍以 §0–§3 的证据为准。

### 6.1 覆盖总览

| 里程碑 | 矩阵 P0/P1 项 | 其中已完成基线 | 本里程碑新增/修复 | v2.0.0 门禁 | 关键门禁 |
|---|---:|---:|---:|---:|---|
| M3a 群聊房间 | 32 | 4 | 28 | 31 | coalescer + 三闸门 + mention + 群目录 + thinking/中间状态 + 注入面 untrusted + 出站证据/媒体边界契约 + codex read-only |
| M3b 权限快照 | 13 | 0 | 13 | 12 | 工具网关 spike 决策门；无网关只承诺 codex 硬边界 |
| M4a 接话 | 8 | 0 | 8 | 8 | all 模式显式开启 + 成本/轮数上限 |
| M4b 记忆摘要 | 15 | 0 | 15 | 15 | 三域隔离 + 四段摘要 + 最近 8 条 |
| M4c 体验补齐 | 8 | 0 | 8 | 8 | 富片段 + 撤回/Reaction + Markdown + CLI |
| M5 发布 | 0 | 0 | 0（另 6 条发布检查） | 0 | P0/P1 清零、迁移回滚、签名公证 |
| **合计** | **76** | **4** | **72** | **74** | — |

> 口径说明：§6 排 §3 矩阵的 76 个 P0/P1；M3a 32 行 = 4 项 M2 基线回归 + 28 项新建；M3b/M4a/M4b/M4c 全部为新建。发布门禁 74 = 76 − 2（L507、L516 两项自有未验证假设）；M5 的 6 条发布检查另列，不计入 74。§5 的 engine 锁拆分、workflow room、Tauri 通知等自有能力作为对应里程碑的工程支撑，不改变本表计数，也不替代矩阵项的验收。

### 6.2 M3a 群聊房间

| 里程碑 | 事项 | 优先级 | 实现要点 | 依赖 |
|---|---|---|---|---|
| M3a | `msg-`+ULID 客户端幂等（§3.1 L489） | P0 | 保留 `UNIQUE(room_id, client_msg_id)`；群聊重放/重复提交复用原 turn，不新增气泡 | 已完成 M2a-2；M3a 回归 |
| M3a | 每聊天串行 lane、跨聊天并行（§3.1 L490） | P0 | coalescer 与 lane 共用同一 actor；每群批只产生一个 turn；跨房间仍并行 | 已完成 M2a-2；M3a 回归 |
| M3a | 私聊隐式房间 + session 续接（§3.2 L504） | P0 | 建群不能回归 `open_dm` 的 get-or-create 与 `room_persona_sessions`；补 DM 回归用例 | 已完成 M1/M2b |
| M3a | typing 指示（§3.6 L588） | P0 | 保留 1.5s 节拍/6s 熄灭；群聊补 persona 名与“思考/工具”两态 | 已完成 M2a-3 |
| M3a | thinking / 中间状态抑制（§3.1 L497） | P0 | 10s/30s 攒批期间显示“思考/调用工具/等待审批”；minimal/normal 抑制压缩、自省、stream stalled、gateway restart 等运行时噪音 | run status |
| M3a | 出站附件本回合工具证据（§3.7 turn_attachments） | P0 | turn 级记录 host hook 的工具调用/结果；只有本回合真实产物可授权附件；`MEDIA:` 散文不构成授权 | turn snapshot |
| M3a | 媒体下载边界（§3.7 media-boundaries） | P0 | 仅 HTTPS；逐跳 DNS 后全为公网单播；锁 IP 再连；最多 5 次重定向；`no_proxy` 直连复检；M4c 落地前复用 | network |
| M3a | 提示注入面 untrusted 标注（§3.7 prompt-injection） | P0 | 群描述、成员便签、未读前情、mention 前情、摘要/记忆块统一标 untrusted；system 只做数据不做指令 | prompt render |
| M3a | 运行中 run 落库（§3.1 L493） | P0 | spawn 前插入 `status=running` run；`complete_turn` upsert 终态；恢复扫描的 interrupted/backfill 分支可达 | schema v2 |
| M3a | 群消息 coalescer（§3.1 L491） | P0 | 纯状态机；idle=10s、max-wait=max(30s,idle)；最后一条为宿主 turn；合并 mention/媒体/发言人 | lane |
| M3a | 忙群 hold（§3.1 L492） | P0 | 上一 turn 未完成继续攒批；空闲后整批 flush；每群最多一个 pending batch | coalescer |
| M3a | 建群/成员/群名（§3.2 L505） | P0 | `create_group/add_member/update_group` + UI；owner persona 必须显式；禁止无主群 | schema v2 |
| M3a | 群资料（§3.2 L506） | P1 | 标题/描述/群主/参与者注入 prompt；群主与 agent owner 分离 | create_group |
| M3a | 每群项目文件夹（§3.2 L507） | P1（按我方设计；不计入发布门禁） | 创建 `~/AgentChatProjects/<群名>` 并绑定 cwd；群会话首轮记录 sandbox | create_group |
| M3a | 禁言 `muted`（§3.2 L508） | P0 | room/member 两级 muted；命令也挡；flush 时再查一次 | settings cache |
| M3a | `reply_mode=all/mention`（§3.2 L509） | P0 | 默认 mention；all 显式开启；未 @ 的消息进台账但不进 turn | target resolver |
| M3a | `batch_delay_seconds` + max-wait（§3.2 L510） | P0 | `batch_delay_s` 默认 10、clamp 1-60；max-wait 至少 30s | coalescer |
| M3a | flush 二次查设置（§3.2 L511） | P0 | 设置版本变化重查 muted/reply_mode/turn_limit；不满足记 no_reply | settings cache |
| M3a | 自动轮数上限（§3.2 L512） | P0 | 人类消息后计数；达上限拒绝并系统提示；新人类消息清零 | turns |
| M3a | 结构化 mention + `@all`（§3.3 L522） | P0 | 消息片段/mentions 表；@ picker；@all 哨兵不展开 | schema v2 |
| M3a | mention routing（§3.3 L523） | P0 | 每帧渲染 addressed_to_current/other/no_structured；文本 @ 不覆盖结构化 | mention |
| M3a | @ 立即 flush（§3.3 L524） | P0 | 绕过合并窗口但尊重 muted/turn_limit；与 pending 批去重 | coalescer |
| M3a | 未读前情 + 字符预算（§3.3 L525） | P1 | 取上次 turn 后未见群消息，正序补前；超预算截断并提示 | turns |
| M3a | 目标解析链（§3.3 L527） | P1 | default/last/owner/指定逐级解析；无效目标给系统提示，不静默丢 | target resolver |
| M3a | 多 @ 每人独立 turn（§3.3 L528） | P1 | 每个被 @ persona 独立 turn，共享 batch id；各自计 turn_limit | schema v2 |
| M3a | codex 群会话 read-only（§3.4 L551） | P0 | 群会话首轮强制 `sandbox=read-only`；续聊 sandbox 不一致直接拒单；DM 现为无条件 read-only，M3a 同步改为 read-only/workspace-write 显式选择（契约 §6.4） | K3 |
| M3a | 无回复令牌（§3.6 L585） | P0 | 解析 `<agentchat:no-reply/>`/`NO_REPLY`；抑制气泡；不落 failed | result parser |
| M3a | 激活问候（§3.6 L580） | P1 | 绑定成功发本地系统问候（不调模型）；失败给下一步 | bind flow |
| M3a | 时间戳/迟到提示（§3.6 L584） | P1 | 存 `sent_at`；UI 显示迟到提示；统一本地时区 | schema v2 |
| M3a | 在线状态诚实化（§3.6 L587） | P1（诚实修复 P0） | 静态绿点改为健康检查或“未知”；不可达显示离线 | doctor |
| M3a | 安装/绑定流程（§3.6 L595） | P1 | 安装→登录→模型→问候四阶段；每阶段可重试、可跳过 | credential |
| M3a | 已读/送达诚实化（§3.1 L496） | P1（诚实修复 P0） | “已读”改“已送达/已回复/失败/未知”；M4c 再接 `last_read_msg_id` | UI |

### 6.3 M3b 权限快照

**前置决策门（建议）**：先做工具网关 spike。能拦截 CLI 工具调用，才谈逐次审批和 host tool guard；不能拦截，就只能承诺 codex 的 OS sandbox 是硬边界，其他引擎标“prompt 级近似”，不能对外宣称完整权限快照（§3.4 L539、L543、L545）。

| 里程碑 | 事项 | 优先级 | 实现要点 | 依赖 |
|---|---|---|---|---|
| M3b | 服务端 owner-permission gate（§3.4 L539） | P0（依赖工具网关） | spike 验证 CLI 进程内工具调用能否拦截；不能则只承诺 codex OS sandbox，其他引擎明确降级 | 工具网关 spike |
| M3b | 每人工具授权 direct/ask/deny（§3.4 L540） | P0 | grants 表写入/查询；非 owner 默认 deny 或 ask；按 persona 记录 | turn snapshot |
| M3b | 每次调用审批（§3.4 L543） | P0（受限） | 有网关才做逐次审批卡；无网关则 prompt+UI 警示，不称强制 | 工具网关 spike |
| M3b | turn 级 grants 快照 + 多发言人交集（§3.4 L544） | P0 | 每 turn 冻结 speakers/grants/owner/origin；多非 owner 发言人取交集 | schema v2 |
| M3b | 宿主工具两层防护（§3.4 L545） | P0 | toolset 覆盖 + pre_tool_call；无 hook 引擎用 workspace 隔离/只读 cwd | 工具网关 spike |
| M3b | 本地附件/凭据 denylist（§3.4 L548） | P1；P0 前置：出站媒体须有本回合工具证据 | canonical path + `.env`/`auth.json`/key/系统路径 denylist；远程附件仅公网单播；前置依赖 turn_attachments 式证据账本 | file picker / turn evidence |
| M3b | per-engine 权限能力矩阵（§3.4 L552） | P0 | codex 硬；dsh/claude/qwen prompt；arkcli/pi/hermes 实测；UI 标“硬/近似/未知” | M3b |
| M3b | 房间权限档 `project/full/readonly`（§3.2 L516） | P1（按我方设计；自有未验证假设，不计入发布门禁） | 定义语义并写快照；明确它不是我方对 ClawChat 官方房间档的复刻 | grants |
| M3b | 选择器 exact/capability/all + 过期（§3.4 L541） | P1 | 优先级 exact>capability>all；同级 deny>ask>direct；支持 expiry | grants |
| M3b | 授权确认卡 + 6 位码 + 10 分钟（§3.4 L542） | P1 | owner DM 发卡；6 位码/10 分钟；批准/拒绝都审计；拒绝立即生效 | grants |
| M3b | owner-only 工具/管理动作（§3.4 L546） | P1 | 改群设置/授权/编排只允许 owner persona；public/all 不能暴露 | grants |
| M3b | 跨 session 路由（§3.4 L547） | P1 | 子 Agent/定时/Kanban 默认拒绝；只允许查看/停止 | grants |
| M3b | 审批/权限审计 UI（§3.4 L553） | P1 | approvals 表 + 时间线；导出谁在何时授权了什么 | approvals |

### 6.4 M4a 接话

| 里程碑 | 事项 | 优先级 | 实现要点 | 依赖 |
|---|---|---|---|---|
| M4a | 群命令模式 + 命令绕过批处理（§3.2 L513） | P1 | `/new /stop /help` 绕过合并窗口但尊重 muted/turn_limit；回执给发起人 | coalescer |
| M4a | 对其它 Agent 的回复策略（§3.2 L514） | P1 | 默认不自动回其他 Agent；@all 不算定向；循环熔断 | turn |
| M4a | 死会话/自回声/入站速率（§3.2 L515） | P1 | 死房间 gate；self-echo 过滤；每房间速率告警 | lane |
| M4a | `all` 档多 Agent 接话（§3.3 L529） | P0 | all 显式开启；群描述当舞台；模型自主决定；受 turn/成本上限 | group prompt |
| M4a | 群描述作为舞台规则（§3.3 L530） | P0 | 注入 group description，优先于默认回复规则；群主可编辑 | schema |
| M4a | `chain_react` 接话轮开关（§3.3 L531） | P1（自有） | 房间级开关；@ 答后最多一轮补充；默认关 | all mode |
| M4a | 接话深度/轮数/成本上限（§3.3 L532） | P0 | 深度/轮数/token/时长上限；超限停并提示 | turn_limit |
| M4a | 团队模板（§3.3 L533） | P1 | hub/worker/checker、writer/reviewer、并行研究；一键建群+角色+目标策略 | persona/group |

### 6.5 M4b 记忆摘要

| 里程碑 | 事项 | 优先级 | 实现要点 | 依赖 |
|---|---|---|---|---|
| M4b | `owner.md`（§3.5 L559） | P0 | MemoryService 唯一写；owner 偏好/禁忌；每回合注入 | schema |
| M4b | `users/<id>.md`（§3.5 L560） | P0 | 每个 persona/联系人便签；只读本人+owner | MemoryService |
| M4b | `groups/<id>.md`（§3.5 L561） | P0 | 群规则/决定；群回合注入 | MemoryService |
| M4b | 结构化 metadata 块（§3.5 L562） | P1 | `<!-- agentchat:metadata -->`；字段白名单；正文/资料分离 | memory |
| M4b | 记忆读范围（§3.5 L563） | P0 | owner/user/group 域限；群只注入 owner+本群+本批发言者 | memory |
| M4b | 回合注入（§3.5 L564） | P0 | owner+group+current speakers；预算裁剪；不串私聊 | memory |
| M4b | append/replace/edit + 原子写（§3.5 L565） | P1 | 段落去重；replace/edit 先读；tmp+rename 原子写 | memory |
| M4b | 压缩/重置前 sediment（§3.5 L566） | P1 | 摘要/reset 前合成 turn；只写可见 note；回复丢弃 | memory |
| M4b | DM 25k / 群 15k 预算（§3.5 L567） | P0 | 字符估算；超限触发；UI 显示预算 | history |
| M4b | 四段摘要 Now/Decided/Open/Index（§3.5 L568） | P0 | 固定四段；不抄密码/长链接；prompt_version 可升级 | summary |
| M4b | 不可变摘要文件 + state CAS（§3.5 L569） | P1 | 不可变 `.md` + state CAS；可回滚/审计 | summary |
| M4b | 最近 8 条 + recent window（§3.5 L570） | P0 | 摘要后保留最近 8 条原文窗口 | summary |
| M4b | idle reset（§3.5 L571） | P1 | DM 24h/群 6h；维护任务；不占 turn | scheduler |
| M4b | 摘要失败退避 + watchdog（§3.5 L572） | P1 | 1/2/4/8 退避；5 分钟 watchdog；旧摘要保留 | summary |
| M4b | 重启中断提示（§3.5 L574） | P1 | 120s 后一次本地化“可能未完成，请重发” | recovery |

### 6.6 M4c 体验补齐

| 里程碑 | 事项 | 优先级 | 实现要点 | 依赖 |
|---|---|---|---|---|
| M4c | Protocol v2 富片段（§3.1 L488） | P1 | 文本/mention/image/file/audio/video parts；文件卡片/语音气泡 | schema |
| M4c | 撤回 tombstone（§3.1 L494） | P1 | 双向 2 分钟撤回；tombstone；上下文/摘要/记忆排除 | schema |
| M4c | Reaction（§3.1 L495） | P1 | 单 emoji 槽；可撤；fire-and-forget | schema |
| M4c | 4000 字符分块 + Markdown（§3.1 L498） | P1 | markdown-aware 分块；代码块/链接；CSP 下安全渲染 | UI |
| M4c | 当前会话历史搜索/读取（§3.5 L573） | P1 | 本地 FTS；around/range/latest；脱敏后展示 | DB |
| M4c | 输出可见性 preset（§3.6 L582） | P1 | minimal/normal/full；抑制压缩/重启等噪音 | run |
| M4c | 媒体/文件/语音（§3.6 L589） | P1 | 图片/文件/语音；截图粘贴；本地路径/上传 | storage |
| M4c | CLI 命令面（§3.6 L594） | P1 | headless `agentchat send/rooms/history`；脚本/CI | Tauri commands |

**EXP/榜单/race 的定位**：三者均为 P2 自有项，不是 ClawChat 官方能力（§3.6 L597-L598；§1.12），并**移出 v2.0.0 验收范围**，不占 M4c/M5 关键路径；设计契约 §9 的 M4c（EXP+动态）与 M5（race+发布）定义需同步修订。若要保留，建议改叫“贡献值/被采纳次数”，且只统计可验证的采纳、复核、交付事件。

### 6.7 M5 发布

| 里程碑 | 事项 | 优先级 | 实现要点 | 依赖 |
|---|---|---|---|---|
| M5 | P0/P1 全量回归 | P0 | 74 项逐条验收；0 个开放 P0；P1 未完成项必须有书面豁免和用户可见说明 | M3a-M4c |
| M5 | schema v1→v2 迁移/回滚 | P0 | `VACUUM INTO` 备份；迁移失败回滚；降级只读；保留最近 5 份 | schema v2 |
| M5 | 安全/隐私文案审计 | P0 | CSP、capabilities、权限矩阵、出机边界文案；删除“绝对不出机”类表述 | M3b |
| M5 | 8 engine 干净机冒烟 | P0 | 8/8 成功或给出明确凭证引导；dmall 回归；记录 CLI 版本 | M3a |
| M5 | 崩溃/断电演练 | P0 | spawn/执行/complete 三阶段 kill -9；不重复副作用；恢复后有提示 | M3a |
| M5 | 签名/公证/自动更新 | P0 | dmg 公证、干净 Mac 双击、updater 回滚 | M5 |

### 6.8 M3a 开工清单（下周可立即执行）

1. **冻结验收表**：把 §6.2 的 32 行（4 项 M2 基线回归 + 28 项新建）写成可勾选清单；先跑 `./smoke.sh --quick`、现有 Rust selftest，记录基线。验收表不通过，不进入建群 UI。
2. **修 running run 落库 + lease**：spawn 前插入 `status=running` run，完成事务 upsert；**恢复扫描必须先判 `lease_until > now` 且 persona 不在 `active` 登记表，才能标 interrupted/rerun**。`recover_scan` 被 `lane_dispatcher` 每 300ms 调用一次，若只落库不加这两个判断，正在执行的 turn 会被每个 tick 标成 interrupted 并重复派发。先修这个，避免群聊崩溃后重复副作用。
3. **schema v2 迁移**：新增 `message_parts`/`mentions`、`messages.reply_to_msg_id`、`messages.sent_at`、`turns.batch_id`、`rooms.settings_version`、`room_members.notify_mode`、`rooms.last_read_msg_id`；`turns.trigger_msg_ids` 语义扩展为 batch 共享（多 @ 同一批）；沿用备份+回滚。
4. **抽出纯函数 Gate/Resolver**：`resolve_targets(room, msg)`、`gate(room, member, msg)`、`turn_limit` 先不接 UI；单测覆盖 muted/reply_mode/@/turn_limit。
5. **实现 coalescer 状态机 + thinking/中间状态抑制**：idle=10s、max-wait=max(30s,idle)、@ 立即 flush、忙群 hold；攒批/执行期间给“思考/调用工具/等待审批”中间态，minimal/normal 抑制运行时噪音；单测覆盖乱序、重复、空批。
6. **群设置面板**：muted/reply_mode/batch_delay/turn_limit/group_prompt；设置版本号 + 保存后 flush 复检。必须先于 mention 闭环，否则 `reply_mode` 默认 `all`，无法复现“mention 未 @ 静默”。
7. **接 lane actor + untrusted 标注**：coalescer → Gate → Resolver → turn 串成一个 actor；每群一个 pending batch；跨房间并行；群描述、成员便签、未读前情、mention 前情、摘要/记忆块进入 prompt 前统一标 untrusted。
8. **建群最小闭环**：`create_group/add_member/update_group` + UI；群目录 `~/AgentChatProjects/<群名>`（自有假设，不计入发布门禁）；codex 群会话首轮 read-only，DM 改为 read-only/workspace-write 显式选择；冻结 turn_attachments 本回合证据与 media-boundaries（HTTPS/公网单播/IP 锁定/重定向上限）契约。
9. **mention 最小闭环**：@ picker、@all、`mentions` 持久化、routing 元数据注入；文本 @ 暂不自动链接。
10. **诚实状态修复**：在线点改“健康检查/未知”；已读改“已送达/已回复/失败/未知”；4 个未探测 engine 改“未探测”；IME 加 `isComposing` 保护。
11. **绑定/凭证四阶段**：安装→登录→模型可用→工作区可写；绑定成功发本地 greeting；失败给可复制命令。
12. **验收**：3 persona 群跑 mention/all/@/turn_limit/忙群 hold；DM 回归；`./smoke.sh`；改会话逻辑跑 `./mem_test.sh`；kill -9 恢复演练。

> v1.1：已吸收第二引擎复核（dsh）11 项修订，结论 FIX→PASS 待确认。
