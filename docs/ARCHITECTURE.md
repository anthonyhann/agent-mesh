# Architecture / 架构说明

## 分层

```
调度层   你的宿主（任意能跑 shell 的 agent/CI/cron）
           │  写任务单 → 调脚本 → 读 result.json
编排层   run_worker.sh（单任务）/ run_pipeline.sh（串行）/ run_dag.py（并行 DAG）
           │  超时看护 · 重试 · 验收 · 落盘 · ledger
工人层   dsh / codex / opencode / hermes / arkcli / claude（CLI 非交互，fresh context）
记录层   results/<id>/{stdout,stderr,answer,result.json} + ledger.jsonl + 流转图 + 看板
```

## 核心设计决策

1. **文件系统是总线**：任务单/产物/流水全部落盘。跨语言、跨权限、可断点续查，天然审计友好。
2. **fresh context 优先**：节点间只传归一化正文（answer.txt），超长自动降级为"文件路径+摘要"——既防截断，又避免把上一个 worker 的上下文腐烂传给下一个。
3. **验收机器化**：`done_criteria.stdout_regex` 对 JSON 信封只匹配正文（content/result 字段），防止 reasoning 文本造成假阳性。
4. **双引擎复核是默认质量门**：重要产出派第二个不同厂商 worker 复核；pipeline 的 n1→n2 结构就是为这个模式而生。
5. **凭据门控**：缺凭证的 worker 秒级明确失败，绝不挂起（见 claude 分支）。

## 目录

```
run_worker.sh / run_pipeline.sh / run_dag.py   编排内核
render_flow.py / mesh_board.py                  可视化（流转图/看板）
publish_result.sh / redact.py                   发布 sink（可选，脱敏内置）
smoke.sh / mem_test.sh / prune.sh               运维三件套
scripts/setup.sh                                环境自检
examples/tasks/                                 任务单样例
examples/skills-governance/                     应用示例：多 agent skills 治理
docs/                                           接入与架构文档
```
