# Pool CLI

`pool-cli` 是 Pool 本地 runtime 的最小命令行入口。它不另建状态机，而是直接复用 `RuntimeHttpServer` handler 读取和写入同一份 SQLite runtime，供 Hermes、Agent CLI、脚本和人工调试使用。

## 目标

- 让 Agent 能用稳定命令读取 Pool 当前状态，而不是只依赖浏览器 UI。
- 让 Hermes / Agent CLI 可以读取 MCP resource、workflow context、节点上下文和运行图。
- 让支持 MCP stdio 的 Agent 直接读取 `pool://` resources，并通过命名 tools 调用 runtime 写操作。
- 让节点级执行可以从 CLI 触发，并继续写入 tasks、assets、provider_requests、software_actions、agent_sessions 和 workflow_events。
- 让 Provider/API Key 接入可以从 CLI 管理，供 AI 图片、3DGS gateway、Suno 等外部服务做本地 smoke。
- 让外部软件控制、三类输出包、Hermes/Agent 会话和桌面识别 controller handoff 可以从 CLI 触发与回填。
- 让 Agent CLI 可以接管运行队列里的审批、取消和重试，而不必切回浏览器或手写 curl。
- 保持本地优先：SQLite 是状态源，本地项目包是资产源。

## 命令

全局参数必须放在子命令前：

```bash
pool-cli [--db <sqlite-path>] [--project <slug|*>] <command>
```

默认读取：

- `POOL_RUNTIME_DB`，未设置时使用 `target/runtime-http-smoke/pool-runtime.sqlite`
- `POOL_PROJECT`，未设置时按 Runtime HTTP 默认 project filter 行为处理

当前子命令：

```bash
pool-cli status
pool-cli snapshot
pool-cli projects
pool-cli resources
pool-cli adapters
pool-cli integration-readiness
pool-cli tasks
pool-cli events --limit 24
pool-cli runtime-budget
pool-cli runtime-preflight
pool-cli runtime-execution-plan
pool-cli runtime-run-next
pool-cli runtime-handoff
pool-cli prd-readiness
pool-cli production-evidence-requirements
pool-cli runtime-graph
pool-cli workflow-context
pool-cli workflow-context <workflow-id>
pool-cli node-context
pool-cli node-context <node-id>
pool-cli mcp pool://tasks
pool-cli mcp pool://adapters
pool-cli mcp pool://integration-readiness
pool-cli mcp pool://provider-contracts/triposplat
pool-cli mcp pool://workflow/<workflow-id>
pool-cli mcp pool://runtime-graph
pool-cli mcp pool://runtime-budget
pool-cli mcp pool://runtime-preflight
pool-cli mcp pool://runtime-execution-plan
pool-cli mcp pool://runtime-handoff
pool-cli mcp pool://prd-readiness
pool-cli mcp pool://production-evidence-requirements
pool-cli mcp pool://node-context/<node-id>
pool-cli serve-mcp
pool-cli api-keys
pool-cli set-api-key openai-image-2 --api-key-env OPENAI_API_KEY
pool-cli adapter-health --software-only
pool-cli provider-contracts triposplat
pool-cli provider-conformance-package worldlabs-marble --output-dir worlds/demo/output
pool-cli integration-conformance-package --output-dir worlds/demo/output
pool-cli integration-conformance-package --provider worldlabs-marble --software resolve --agent-kind all --output-dir worlds/demo/output
pool-cli software-contracts unreal
pool-cli software-conformance-package resolve --output-dir worlds/demo/output
pool-cli agent-conformance-package all --output-dir worlds/demo/output
pool-cli unreal-mcp-bridge
pool-cli provider-health openai-image-2 --api-key-env OPENAI_API_KEY
pool-cli run-provider world-labs-marble --execution-mode mock --no-approval --prompt "CLI provider smoke"
pool-cli production-evidence-provider-matrix target/provider-evidence-matrix --no-env
pool-cli production-evidence-run-plan target/provider-run-plan.json --output-root target/provider-evidence-matrix
pool-cli production-evidence-software-matrix target/software-evidence-matrix --no-env
pool-cli production-evidence-software-matrix target/software-evidence-matrix --production-software --evidence-bundle target/software-evidence-matrix/software-production-evidence-bundle.json
pool-cli production-evidence-requirements
pool-cli production-evidence-tasks
pool-cli production-evidence-claim provider:midjourney:production_upstream --assignee worker-1 --role provider_worker --output-root target/production-evidence
pool-cli production-evidence-handoff target/production-evidence/handoff.json --output-root target/production-evidence
pool-cli production-evidence-handoff-package --output-dir worlds/demo/output --output-root worlds/demo/output/production-evidence --include-snapshot
pool-cli prd-completion-package --output-dir worlds/demo/output --include-snapshot
pool-cli production-evidence-template target/production-evidence/bundle.json --output-root target/production-evidence
pool-cli production-evidence-item-template provider midjourney target/production-evidence/midjourney-item.json --output-root target/production-evidence
pool-cli merge-production-evidence target/production-evidence/combined.json target/provider-evidence.json target/software-evidence.json target/desktop-vision-evidence.json
pool-cli validate-production-evidence docs/examples/production-evidence-bundle.example.json
pool-cli import-production-evidence <真实生产证据bundle>
pool-cli validate-production-evidence-item <单项生产证据item.json>
pool-cli submit-production-evidence-item <单项生产证据item.json>
pool-cli provider-request-metadata <provider-request-id>
pool-cli provider-gateway-worker-contract
pool-cli provider-gateway-worker --once
pool-cli worker-self-checks --output-root target/pool-worker-self-checks
pool-cli unreal-mcp-bridge-worker --once --output-root worlds/demo/output
pool-cli unreal-mcp-bridge-worker --bind 127.0.0.1:8790 --output-root worlds/demo/output
pool-cli hermes-mcp-bridge-worker --once --output-root worlds/demo/output
pool-cli software-api-bridge-worker resolve --once --output-root worlds/demo/output
pool-cli hermes-mcp-bridge-worker --bind 127.0.0.1:8792 --output-root worlds/demo/output
pool-cli software-api-bridge-worker resolve --bind 127.0.0.1:8793 --output-root worlds/demo/output
pool-cli software-health blender --priority SkillsCli
pool-cli run-software blender --action execute-cli --priority SkillsCli --payload-json '{"command":"/bin/echo blender-ok","allowed_commands":["/bin/echo","echo"]}'
pool-cli output-package --node-id outputs --source-asset worlds/demo/output/1-world.glb --duration-ms 12000
pool-cli handoff-package --node-id agent --output-dir worlds/demo/output --include-snapshot
pool-cli agent-session agent-cli --command-id echo --command "/bin/echo pool-ok" --tool cli --execute --allowed-command /bin/echo
pool-cli agent-transcript <agent-session-id>
pool-cli agent-stream <agent-session-id> --limit 24
pool-cli desktop-contract
pool-cli desktop-requests
pool-cli desktop-run-next --controller-id local-vision-dry-run --status succeeded
pool-cli desktop-result <software-action-id> --status succeeded --message "desktop controller finished"
pool-cli runtime-run-next --node-id <node-id>
pool-cli runtime-run-next --task-id <task-id> --execute --allow-approval
pool-cli run-node <node-id> --execution-mode mock --prompt "CLI smoke 3DGS run"
pool-cli run-workflow --agent-mode stage --three-dgs-mode mock --unreal-mode mock --prompt "CLI content burst"
pool-cli approve-task <task-id>
pool-cli cancel-task <task-id>
pool-cli retry-task <task-id>
```

`run-node` 会调用 `POST /api/nodes/run`，Provider 节点进入 Provider runner，软件节点进入 SoftwareActionRunner，Agent/Hermes 节点进入 AgentSessionRunner，输出节点进入 OutputPackageRunner。节点运行会把 node context 中的 `control_context` 写入 Provider request ledger 或软件 action payload，供后续审批、重试和 Agent/Hermes 审计复用。

`adapters` 会调用 `/api/adapters`，读取 Provider/软件 adapter catalog、Provider alias、控制优先级和本地优先策略，供 Agent 在调用 health/run 前选定正确 provider_id、adapter_id 与控制路径。

`integration-readiness` 会调用 `/api/integration-readiness`，读取同源 `pool://integration-readiness` readiness 矩阵。它把 adapter catalog、API key、Provider 请求、软件动作、任务状态和 Agent session 汇总成 `ready / needs_configuration / needs_execution / needs_attention`，并额外输出 5 人团队 lane、每行 next action 和按优先级排序的 run plan，适合在生成或执行 conformance package 前先定位缺配置、缺执行和失败项。MCP tool `pool_integration_readiness` 返回同一份只读 payload。

`provider-contracts [provider-id]` 会调用 `/api/provider-contracts`，读取机器可读 Provider 接入合同。它会说明 Pool runtime 的 `/api/provider-runs` body、通用 AI media gateway / 3DGS gateway 的 submit/poll path、状态字段、输出字段、环境变量和本地文件优先策略；不传 provider id 时返回全部合同。`pool-cli mcp pool://provider-contracts/<provider-id>` 返回同一份资源，供 Hermes/Agent 在实现本地 gateway 或厂商 SDK adapter 前读取。

`provider-conformance-package <provider-id>` 会调用 `POST /api/provider-conformance-packages`，把单个 AI/3DGS Provider 的 contract、provider gateway worker contract 和真实上游接入 runbook 写成本地交接包。默认输出到 `<output-dir>/control/provider-conformance/<provider-id>/`，包含 `.1-provider-conformance-package-request.json`、`1-provider-contract.json`、`2-provider-gateway-worker-contract.json`、`3-provider-conformance-runbook.json`、`4-provider-conformance-preflight.json`、`5-provider-conformance-runner.sh` 和 `6-provider-conformance-package-manifest.json`。runner 支持 `--preflight` 检查 endpoint、upstream、API key 和 production attestation env，`local` 只跑 Pool gateway worker baseline，`run` 在预检通过后顺序执行 real upstream worker、provider health/smoke、production matrix 和 validate/import。MCP tool `pool_provider_conformance_package` 走同一入口，适合 Hermes/Agent 给 Midjourney、Nano Banana Pro、Suno、Marble、TripoSplat、SAM-3D、Spark、群核科技等具体 Provider worker 分发接入任务。

`integration-conformance-package` 会调用 `POST /api/integration-conformance-packages`，一次性写出 Provider、软件 adapter 和 Agent/Hermes 的总验收包。默认输出到 `<output-dir>/control/integration-conformance/`，并默认覆盖 9 个 required Provider、11 个 required software adapter 与 `agent_kind=all`。可用 `--provider` / `--software` 缩小范围做局部 smoke，用 `--no-providers`、`--no-software` 或 `--no-agent` 排除某一类。总包包含顶层 request、runbook、runner script、manifest，以及每个子包自己的 contract、runbook、preflight、runner 和 manifest；MCP tool `pool_integration_conformance_package` 走同一入口，适合 5 人团队一次性分派 AI/3DGS、软件控制和 Agent/Hermes 接入验收。

`software-contracts [adapter-id]` 会调用 `/api/software-contracts`，读取机器可读外部软件控制合同。它会说明 Pool runtime 的 `/api/software-health` body、`/api/software-actions` body、API/MCP、Skills/CLI、Desktop Recognition、Human Takeover 优先级链、支持的 action kind、环境变量和 fallback policy；不传 adapter id 时返回全部合同。`pool-cli mcp pool://software-contracts/<adapter-id>` 返回同一份资源，供 Hermes/Agent/桌面 controller 在控制 Unreal、Blender、Resolve、TouchDesigner、MadMapper、Unity、Nuke 或 Hermes 前读取。

`software-conformance-package <adapter-id>` 会调用 `POST /api/software-conformance-packages`，把单个软件 adapter 的控制合同和 `conformance_runbook` 写成本地交接包。默认输出到 `<output-dir>/control/software-conformance/<adapter-id>/`，包含 `.1-software-conformance-package-request.json`、`1-software-control-contract.json`、`2-software-conformance-runbook.json`、`3-software-conformance-preflight.json`、`4-software-conformance-runner.sh` 和 `5-software-conformance-package-manifest.json`。runner 支持 `--preflight` 检查 upstream endpoint、production endpoint/artifact/attestation env，`local` 只跑本地 bridge worker baseline，`run` 在预检通过后顺序执行 real upstream bridge、health/action smoke、production matrix 和 validate/import。MCP tool `pool_software_conformance_package` 走同一入口，适合 Hermes/Agent 给 Resolve、Blender、Unreal、TouchDesigner 等具体操作者分发接入任务。

`agent-conformance-package [all|hermes|agent-cli]` 会调用 `POST /api/agent-conformance-packages`，把 Agent session 控制合同、Hermes/Agent CLI 验收 runbook、preflight、runner script 和 manifest 写成本地交接包。默认输出到 `<output-dir>/control/agent-conformance/<kind>/`，包含 `.1-agent-conformance-package-request.json`、`1-agent-session-contract.json`、`2-agent-conformance-runbook.json`、`3-agent-conformance-preflight.json`、`4-agent-conformance-runner.sh` 和 `5-agent-conformance-package-manifest.json`。runner 支持 `--preflight`、`local` 和 `run`；`local` 会跑 Hermes bridge worker baseline 与 Agent CLI allowlist smoke，`run` 会按合同 staging Hermes session、执行 Hermes HTTP、staging Agent CLI、执行 allowlist CLI，并要求 `POOL_HERMES_ENDPOINT`。MCP tool `pool_agent_conformance_package` 走同一入口，适合把 Hermes 内嵌控制和 Agent CLI 接入验收分派给 Agent 或具体操作者。

`unreal-mcp-bridge` 会调用 `/api/unreal-mcp-bridge`，读取 Unreal 插件或本地 gateway 侧需要实现的 bridge contract。它固定 `pool_unreal_action`、`mcp_payload`、默认 `/health` / `/mcp` transport、Unreal tool contracts、响应字段和本地 artifact policy；`pool-cli mcp pool://unreal-mcp-bridge` 和 MCP tool `pool_unreal_mcp_bridge` 返回同一份合同。

`unreal-mcp-bridge-worker` 会启动本地 Unreal bridge worker。默认 dry-run 模式只校验 Pool wrapper 并写入 `output/control/unreal-mcp-bridge/*-request.json` / `*-response.json`；加 `--once` 会直接执行 health + dry-run action 自检并退出，不占用常驻端口；加 `--upstream <url>` 后会先校验和落地审计文件，再把同一 body 转发给真实 Unreal 插件或 gateway。可用 `POOL_UNREAL_MCP_ENDPOINT=http://127.0.0.1:8790` 让 `UnrealMcpAdapter` 指向该 worker。

`hermes-mcp-bridge-worker` 会启动本地 Hermes bridge worker。默认 dry-run 模式只校验 `pool_hermes_action` / `mcp_payload` wrapper 并写入 `output/control/hermes-mcp-bridge/*-request.json` / `*-response.json`；加 `--once` 会直接执行 health + dry-run action 自检并退出，不占用常驻端口；加 `--upstream <url>` 后会先校验和落地审计文件，再把同一 body 转发给真实 Hermes MCP/gateway。可用 `POOL_HERMES_MCP_ENDPOINT=http://127.0.0.1:8792` 让 `HermesMcpAdapter` 指向该 worker。

`software-api-bridge-worker <adapter-id>` 会启动通用软件 API/MCP bridge worker。默认 dry-run 模式校验 `pool_software_action` / `mcp_payload` wrapper，并写入 `output/control/software-api-bridge/<adapter-id>/*-request.json` / `*-response.json`；加 `--once` 会直接执行 health + dry-run action 自检并退出；加 `--upstream <url>` 后可作为 Blender、Resolve、Unity、TouchDesigner、MadMapper、Nuke 等真实插件或 gateway 的前置审计代理。可用 `POOL_RESOLVE_ENDPOINT=http://127.0.0.1:8793` 这类 `POOL_<ADAPTER>_ENDPOINT` 让 `GenericSoftwareApiAdapter` 指向该 worker。

`set-api-key` 和 `api-keys` 会调用 `/api/api-keys`。CLI 会把明文 key 传给 runtime credential backend，返回结果只显示脱敏 `key_hint`；设置 `POOL_CREDENTIAL_STORE=keychain` 时写入 macOS Keychain，设置 `POOL_CREDENTIAL_PASSPHRASE` 时写入 AES-256-GCM 封装，未设置时保留 legacy SQLite 兼容模式。推荐用 `--api-key-env`，避免把 key 留在 shell history。`set-api-key --rotation-days <days>` 会把单 key 轮换周期写入 metadata，`api-keys --rotation-days <days>` 会返回 backend/source/owner/age/rotation_due 审计摘要。`runtime-budget` 会读取 `/api/runtime-budget`，用于检查 Provider Key 配置比例、待审批 token、Agent 预算余量和 Provider 请求摘要。`runtime-preflight` 会读取 `/api/runtime-preflight`，用于让 Agent/Hermes 在运行前看到阻塞项、警告、`worker-self-checks` / `pool_worker_self_checks` 本地 worker smoke 和建议 CLI next actions。`runtime-execution-plan` 会读取 `/api/runtime-execution-plan`，按 workflow 拓扑列出可执行步骤、审批门、Provider/软件合同和推荐 CLI/MCP 动作。`runtime-run-next` 会调用 `POST /api/runtime-execution-plan/run-next`，默认 preview 下一步或指定 node/task；只有传 `--execute` 才会实际分派，审批步骤还必须传 `--allow-approval`。`runtime-handoff` 会读取 `/api/runtime-handoff`，把下一步审批、重试、补 Key、本地 worker smoke、桌面识别、可运行节点和 5 人团队角色绑定整理成机器可读执行 runbook。`prd-readiness` 会读取 `/api/prd-readiness`，把 Pool 总体规划拆成要求级 `ready` / `partial` / `blocked` 审计、证据、缺口和 next actions。`prd-completion-gate` 会读取 `/api/prd-completion-gate` 并只输出完成门槛 wrapper；加 `--require-complete` 时如果当前 snapshot 尚未满足完成门槛，会输出 `prd_completion_gate_incomplete` 并以非零退出，适合 CI/Hermes 收口脚本；MCP tool `pool_prd_completion_gate` 支持同名 `require_complete` 参数。`prd-completion-package` 会调用 `POST /api/prd-completion-package`，把当前 readiness、completion gate、production evidence requirements、manifest 和可选 snapshot 写到 `control/prd-completion/`，并同步入 task、assets 和 events；MCP tool `pool_prd_completion_package` 暴露同一 POST 入口给 Agent/Hermes 物化证明包；它只归档当前状态，不替代真实生产证据导入。`production-evidence-requirements` 会读取 `/api/production-evidence/requirements`，把剩余真实 Provider、软件侧和桌面视觉生产证据要求整理成可执行清单，供 Agent 在生成 template、validate 或 import 前先确认缺口。

`provider-health` 会调用 `POST /api/provider-health`，只检查 adapter 配置，不创建任务。

`run-provider` 会调用 `POST /api/provider-runs`，直接调度 ComfyUI、Kling、OpenAI image-2、Midjourney、Nano Banana Pro、Suno、3DGS gateway 或本地 mock 3DGS，并继续写入 `tasks`、`provider_requests`、`assets` 和 `workflow_events`。

`production-evidence-provider-matrix [output-root]` 会把 9 个 required Provider 的真实运行证据矩阵提升为正式 CLI 入口：Midjourney、Nano Banana Pro、Suno 使用 AI media gateway，World Labs Marble、TripoSplat、SAM-3D、Spark、群核科技使用 3DGS gateway，OpenAI image-2 使用 native OpenAI Images adapter。默认会读取 `POOL_MEDIA_GATEWAY_ENDPOINT`、`POOL_3DGS_GATEWAY_ENDPOINT`、`POOL_OPENAI_ENDPOINT`、`OPENAI_API_KEY` 和 `POOL_PROVIDER_PRODUCTION_ATTESTATION`；`--provider-endpoint provider=url` 或 `--provider-endpoint-env provider=ENV_NAME` 可让某个 Provider 使用独立真实 SDK/HTTP worker，也会自动读取 `POOL_PROVIDER_ENDPOINT_<PROVIDER_ID>` / `POOL_<PROVIDER_ID>_ENDPOINT`；`--provider-api-key provider=token` 或 `--provider-api-key-env provider=ENV_NAME` 会为单个 Provider 注入 bearer token，写出的生产证据 metadata 会把 inline `api_key` 脱敏；`--provider-attestation provider=run` 或 `--provider-attestation-env provider=ENV_NAME` 可给单个 Provider 绑定真实上游 worker/SDK run 证明，也会自动读取 `POOL_PROVIDER_PRODUCTION_ATTESTATION_<PROVIDER_ID>` / `POOL_<PROVIDER_ID>_PRODUCTION_ATTESTATION`，缺省再回退到全局 `--production-attestation` / `POOL_PROVIDER_PRODUCTION_ATTESTATION`。传 `--no-env` 可只做安全 dry-run，不接触本机配置。只有加 `--production-upstream` 且对应 Provider 有非占位 attestation 时，成功的真实上游结果才会整理成 `providers[]` bundle；全局 attestation 可覆盖完整矩阵，没有全局 attestation 时每个 required Provider 都必须有自己的 per-provider attestation。该 bundle 仍需经过 `validate-production-evidence` / `closeout-production-evidence` 后才能导入。

`production-evidence-software-matrix [output-root]` 会把 11 个 required software adapter 的真实运行证据矩阵提升为正式 CLI 入口：Unreal、Blender、ComfyUI、DaVinci Resolve、Unity、TouchDesigner、MadMapper、Nuke、动捕数据库、剪辑软件和 Hermes。默认读取 `POOL_UNREAL_MCP_ENDPOINT`、`POOL_HERMES_MCP_ENDPOINT`、`POOL_SOFTWARE_<ADAPTER>_ENDPOINT`、`POOL_<ADAPTER>_ENDPOINT`、`POOL_SOFTWARE_<ADAPTER>_COMMAND`、`POOL_<ADAPTER>_COMMAND`、`POOL_SOFTWARE_<ADAPTER>_ARTIFACTS`、`POOL_<ADAPTER>_ARTIFACTS`、`POOL_SOFTWARE_PRODUCTION_ATTESTATION` 和 per-adapter attestation；也可用 `--software-endpoint id=url` / `--software-endpoint-env id=ENV_NAME`、`--software-command id=cmd` / `--software-command-env id=ENV_NAME`、`--software-artifact id=path` / `--software-artifacts-env id=ENV_NAME`、`--software-attestation id=run` / `--software-attestation-env id=ENV_NAME` 对单个 adapter 显式注入真实控制入口、产物和证明。`resolve` 支持 `davinci-resolve` 别名，`touchdesigner` 支持 `touch-designer`，`motion-db` 支持 mocap/motion database 别名。传 `--no-env` 可只做安全 dry-run，不触发本机软件。只有加 `--production-software` 且具备真实 endpoint 或 command、attestation 和本地 artifact 时，成功结果才会整理成 `software_actions[]` bundle；该 bundle 仍需经过 `validate-production-evidence` / `closeout-production-evidence` 后才能导入。

`production-evidence-desktop-vision [output-root]` 会把已排队的 desktop recognition 请求回填成正式桌面视觉生产证据：外部视觉/OCR/screen controller 先生成本地 trace 文件并给出真实 external action id，CLI 再用 `--production-vision --trace <path> --external-action-id <id>` 和 `POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 或 `--production-attestation <id>` 调用 `/api/desktop-recognition/results`，随后从 ledger 整理出 `desktop_vision[]` bundle。也可用 `--trace-env ENV_NAME`、`--controller-id-env ENV_NAME`、`--external-action-id-env ENV_NAME` 和 `--production-attestation-env ENV_NAME` 显式声明 runner 应读取哪些环境变量，便于 handoff/run-plan 交给外部视觉 controller 或 Hermes 执行。缺少 trace、external action id、attestation 或 trace 文件不存在时只写空 bundle 和诊断，不会把本地 dry-run 或弱回填冒充成外部视觉生产证据。

`provider-gateway-worker-contract` 会读取 `/api/provider-gateway-worker`，返回本地 gateway worker 的 CLI、route、upstream、endpoint env 和 Pool adapter 使用合同。`provider-gateway-worker` 会启动本地 AI media/3DGS HTTP forwarder；加 `--once` 会启动内置 mock upstream，执行 health、AI media submit/poll 和 3DGS submit/poll 自检后退出。它复用 `ProviderGatewayWorker`，接收 Pool gateway contract 请求，通过模板翻译后转发到真实厂商 worker、官方 SDK 包装服务或本地 mock upstream。`provider-sdk-worker-template` 会启动可运行的上游 SDK wrapper 样板，校验 `local_input_manifest`、写入 `1-sdk-worker-request.json` 并返回 Pool-compatible `job_id/status/outputs`，用于真实 SDK 接入前验证边界；该模板输出不能作为生产证据。两个命令都不读写 SQLite，适合作为 Hermes/Agent CLI 启动真实 gateway upstream 或 SDK wrapper 样板的固定入口。

`worker-self-checks` 会一次性运行 Provider gateway worker、Provider SDK worker template、Unreal MCP bridge worker、Hermes MCP bridge worker 和通用 software API bridge worker 的 in-process 自检，并输出机器可读 JSON 报告。它不读写 SQLite，默认把各 worker 的 dry-run 审计文件写入 `target/pool-worker-self-checks`，适合作为 Hermes/Agent CLI 在接入真实 AI 图片、3DGS 和外部软件控制前的统一 smoke gate。

`production-evidence-requirements` 会返回 `pool_production_evidence_requirements`，列出 required Provider、required software adapter、外部视觉 controller 的当前状态、缺失项、必填 bundle 字段、本地文件策略和推荐 CLI 命令。它不写入 SQLite，适合在 `production-evidence-template` 前让 Hermes/Agent/operator 对齐真实厂商 job id、本地 artifact、metadata 和 external visual model trace 需求。

`production-evidence-tasks` 会调用 `GET /api/production-evidence/tasks`，只返回当前缺失生产证据的任务队列，包含 provider/software/desktop vision 的 `task_id`、`kind`、`target_id`、必填字段、artifact policy、item template 命令和 submit item 命令。它适合外部 worker 先领取单个任务，再调用 `production-evidence-item-template --task-id <id>` 生成只属于该任务的 item JSON。

`production-evidence-claim <task-id>` 会调用 `POST /api/production-evidence/tasks/claim`，把一个只读缺口 evidence task 转成可追踪 runtime task，并写出本地 `control/production-evidence/claims/*-claim.json`。常用参数包括 `--assignee`、`--role`、`--output-root` 和 `--source`。响应会返回 claim file、runtime task、item template、`validate-production-evidence-item` 和 `submit-production-evidence-item` 命令；这一步不导入生产证据，只记录任务领取和执行交接。

`production-evidence-handoff [handoff.json] --output-root <dir>` 会读取 `GET /api/production-evidence/handoff`，生成 missing-only 生产证据交付包：包含当前 requirements、`evidence_tasks`、缺口 bundle、validate/import/readiness 命令、HTTP/MCP 入口和 operator checklist。传入 `handoff.json` 时 CLI 会把完整 handoff JSON 写到本地，适合交给 Hermes、外部 Provider worker、软件操作员或视觉 controller operator 收口真实生产证据。

`production-evidence-handoff-package --output-dir <dir> --output-root <dir>` 会调用 `POST /api/production-evidence/handoff-packages`，把 requirements、task queue、missing-only handoff、run-plan、bundle、每个缺口任务的 `items/*-item.json`、隐藏 item-template provenance、可执行 runner script 和 runner preflight 合同写到 `<output-dir>/control/production-evidence/`，并把这些本地文件同步入 assets、tasks 和 events。固定文件顺序包含 `4-production-evidence-run-plan.json`、`5-production-evidence-bundle.json`、`6-production-evidence-package-manifest.json`、`7-production-evidence-runner.sh` 和 `8-production-evidence-runner-preflight.json`，`--include-snapshot` 时额外写 `9-runtime-snapshot.json`。runner script 的 Provider 阶段可用 `POOL_MEDIA_GATEWAY_ENDPOINT` / `POOL_3DGS_GATEWAY_ENDPOINT` 覆盖 AI media/3DGS 家族，也可为每个 required Provider 提供 per-provider endpoint；OpenAI image-2 还需要 `OPENAI_API_KEY` 或 provider-specific key env；production attestation 可用全局值覆盖完整矩阵，或每个 required Provider 提供自己的 per-provider attestation。run-plan 命令会为 9 个 required Provider 显式生成 `--provider-endpoint-env`、`--provider-api-key-env` 和 `--provider-attestation-env`。软件阶段需要 Unreal/Hermes endpoint、各软件显式 `POOL_*_ENDPOINT` 或 `POOL_*_COMMAND`、`POOL_SOFTWARE_PRODUCTION_ATTESTATION` 或 per-adapter production attestation，以及每个 adapter 的 `POOL_*_ARTIFACTS` 本地文件路径；通用 endpoint 可指向转发到真实插件/gateway 的 `pool-cli software-api-bridge-worker <adapter-id>`；也可按 run-plan 命令把这些配置转成 `--software-endpoint-env`、`--software-command-env`、`--software-artifacts-env` 和 `--software-attestation-env` 显式传给软件矩阵。桌面视觉阶段同样可通过 `--trace-env`、`--external-action-id-env` 和 `--production-attestation-env` 固定读取真实外部视觉 controller 的环境变量。可先运行 `7-production-evidence-runner.sh --preflight` 做环境预检；merge/closeout 阶段优先用 PATH 上的 `pool-cli`，找不到时 fallback 到 `cargo run -q -p pool-cli --`，可用 `POOL_CLI_CMD` 覆盖，只有实际使用 cargo fallback 时才要求本机有 cargo；默认执行 Provider/software matrix、merge 和 closeout preflight；设置 `POOL_RUN_DESKTOP_VISION=1` 且提供 `POOL_DESKTOP_VISION_TRACE`、`POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID` 与 `POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 才执行桌面视觉阶段，设置 `POOL_IMPORT_PRODUCTION_EVIDENCE=1` 才执行 closeout import。这个命令适合把真实生产证据任务交给外部 worker/operator/controller 离线执行；默认包含 item 文件，`--no-items` 可只写总包。

桌面视觉生产证据阶段需要外部 controller 先把视觉/OCR/screen trace 落成本地文件，并通过 `production-evidence-desktop-vision` 回填。只有 `POOL_DESKTOP_VISION_TRACE` 指向存在的本地文件、`POOL_DESKTOP_VISION_EXTERNAL_ACTION_ID` 标识真实外部 action、且 `POOL_DESKTOP_VISION_PRODUCTION_ATTESTATION` 标识真实 controller/model run 时，runner 才会写出可导入的 `desktop_vision[]` 生产证据项；这些变量也可以通过 `--trace-env`、`--external-action-id-env` 和 `--production-attestation-env` 显式指定为其他环境变量名。

`production-evidence-template [bundle.json] --output-root <dir>` 会读取 `GET /api/production-evidence/template`，生成覆盖 9 个 required Provider、11 个软件 adapter 和外部视觉 controller 的生产证据 bundle 脚手架；追加 `--missing-only` 时只生成当前 runtime 仍缺失的 Provider、软件 adapter 和 desktop vision 项。传入 `bundle.json` 时 CLI 会把响应中的 `.bundle` 写到该路径，同时完整响应仍会显示 `scope`、`artifact_plan`、`operator_checklist` 和 validate/import 命令。脚手架使用 `replace-with-real-*` external id、Provider/软件/桌面视觉 `production_attestation` 占位，直接 validate 会被拒绝且 `writes:0`；外部 worker、软件插件或视觉 controller 必须先替换真实 id/attestation 并把 Provider artifacts/metadata、software artifacts、desktop trace/artifacts 落成本地文件。

`production-evidence-item-template <kind> <target-id> [item.json]` 会读取 `GET /api/production-evidence/item-template`，生成单个 `submit-production-evidence-item` 可用的 item JSON；也可用 `production-evidence-item-template --task-id <id> [item.json]` 直接从 `production-evidence-tasks` 的任务 id 生成。`kind` 支持 `provider`、`software_action`、`desktop_vision`。传入 `item.json` 时 CLI 会把响应中的 `.item` 写到本地；该 item 仍包含占位 external id 和本地文件路径计划，必须替换为真实生产证据并确保文件存在后才能 submit。只读 MCP resource `pool://production-evidence-item-template` 会列出每个 task 的模板 URI，`pool://production-evidence-item-template/<task-id>` 会返回同一类单项模板 wrapper，适合 Agent/Hermes 在不写文件、不触发导入的情况下分发任务。

`production-evidence-item-from-ledger --provider-request-id <id> [item.json]`、`--software-action-id <id>` 或 `--desktop-vision-action-id <id>` 会读取 `GET /api/production-evidence/item-from-ledger`，把已存在的 `provider_requests`、`software_actions` 或 desktop recognition callback 账本记录整理成 `submit-production-evidence-item` 草稿。响应会返回 `validation.valid`、`artifact_files.complete` 和 `production_flags.complete`；只有三者都为 true 时 `ready_for_import` 才为 true。本地 mock run 默认会保留 `production_upstream:false` / `production_software:false` / `external_visual_model:false` 和 `local_mock_*:true` / `local_trace_smoke:true`，因此不会被当作真实生产证据。desktop vision 草稿要求外部 controller 回填本地 trace 文件，并显式设置外部视觉模型证据。

`production-evidence-bundle-from-ledger [bundle.json]` 会读取 `GET /api/production-evidence/bundle-from-ledger`，把当前 runtime ledger 中已经通过 schema、本地文件和 production flags 三重预检的 Provider、软件动作和 desktop vision 记录批量整理成 production evidence `.bundle`。传 `bundle.json` 时 CLI 会把响应里的 `.bundle` 写到本地；传 `--include-incomplete` 会在响应中保留未 ready 的账本诊断，便于 Hermes/Agent 继续补真实 job id、本地 artifact 或外部视觉 trace。该命令不写 SQLite，输出仍应先走 `validate-production-evidence` 或 `closeout-production-evidence`。

`production-evidence-run-plan [run-plan.json]` 会读取 `GET /api/production-evidence/run-plan`，把当前缺失生产证据整理为真实执行计划：Provider evidence matrix、software evidence matrix、desktop vision controller、merge、closeout preflight、closeout import 和 completion proof。可传 `--output-root <dir>` 指定三路 bundle 与 merged bundle 的输出目录，传 `--source <label>` 标记来源。传入 `run-plan.json` 时 CLI 会保存完整响应，供 Hermes/Agent 或人工 operator 按阶段执行；该命令只读，不写 SQLite。

`merge-production-evidence <combined.json> <bundle.json>...` 是本地文件合并命令，不访问 SQLite。它会把 `production-evidence-provider-matrix`、`production-evidence-software-matrix` 和 `production-evidence-desktop-vision` 输出的 `providers[]`、`software_actions[]`、`desktop_vision[]` 合成一个 bundle，并拒绝不同 `project_slug` 混入同一合并包。Runtime HTTP 同步提供 `POST /api/production-evidence/merge`，MCP stdio 暴露 `pool_merge_production_evidence`，供 Hermes/Agent 直接合并内存中的多路 bundle；它同样返回 `writes:0`，不替代校验。输出的 `combined.json` 或 HTTP `.bundle` 仍应先走 `validate-production-evidence`，再走 `import-production-evidence`。

`closeout-production-evidence [--output merged.json] [--import] <bundle.json>...` 会调用 `POST /api/production-evidence/closeout`。默认只把多个 bundle 合并并执行 validate，返回 `writes:0`、`ready_for_import`、coverage 和本地文件预检；传 `--output` 时会把响应里的 `merge.bundle` 写成本地 merged bundle。只有加 `--import` 时才会显式进入正式导入路径，继续复用本地文件、占位 id、远程 artifact URL 和外部视觉模型校验。加 `--completion-package` 或 `--completion-package-output-dir <dir>` 会在导入成功且 completion gate ready 后自动写出 PRD completion package；可配 `--completion-package-node-id`、`--completion-package-title`、`--completion-package-source` 和 `--no-completion-package-snapshot`。`cargo run -p pool-core --example closeout_production_evidence_bundle -- target/production-evidence-closeout-smoke` 会把示例证据拆成三路 bundle，验证 closeout 预检和显式导入后 PRD readiness 达到 10/10。

Runtime HTTP 还提供 `POST /api/production-evidence/closeout`，MCP stdio 暴露 `pool_closeout_production_evidence`。该工具接收同样的 `bundles[]`，默认只执行 merge + validate 并返回 `writes:0`、`ready_for_import`、合并包和校验结果；只有显式传 `import:true` 时才会复用正式 `import-production-evidence` 路径写入账本。导入成功后，closeout 顶层会返回 `completion_gate`、`ready_for_completion`、`prd_overall_status`、`prd_summary`，并给出 `prd-completion-gate --require-complete` 与 `prd-completion-package` 证明命令；传入 `completion_package:{...}` 时会复用 `/api/prd-completion-package` 写出本地证明包并同步 task/assets/events。本地文件缺失、模板 id、远程 artifact URL 或本地 mock desktop trace 仍会被导入层拒绝。

`validate-production-evidence <bundle.json>` 会调用 `POST /api/production-evidence/validate`，只校验外部生产证据 bundle，不写入 SQLite。它会检查模板占位 external id、Provider/软件/桌面视觉 `production_attestation` 是否存在且非占位、Provider artifact/metadata、software artifact 和 desktop vision artifact 是否为本地路径、desktop trace 是否显式声明外部视觉模型、必填字段和空 bundle，并返回 `writes:0`、各类 evidence 计数、canonical Provider/adapter id、原始输入 id、`artifact_files` 本地文件存在性预检和 PRD 生产证据 coverage 缺口，适合 Hermes/Agent/operator 在真正导入前 dry-run。

`validate-production-evidence-item <item.json>` 会调用 `POST /api/production-evidence/items/validate`，把单个 Provider、软件动作或 desktop vision item 包装成同一套生产证据校验路径，但保持 `writes:0`，不写 `provider_requests`、`software_actions` 或事件流。它适合外部 worker/operator/controller 在正式 `submit-production-evidence-item` 前检查占位 id、本地 artifact/metadata/trace 文件和外部视觉模型标记；MCP stdio 同步暴露 `pool_validate_production_evidence_item`。

`import-production-evidence <bundle.json>` 会调用 `POST /api/production-evidence`，导入外部真实生产运行已经完成后的 Provider、软件动作和桌面视觉证据。导入不会重新执行昂贵任务；它会把 Provider/adapter 别名标准化后写入 `provider_requests`、`software_actions`、tasks 和 events，并在 evidence 中保留 `input_provider_id` / `input_adapter_id` 与 Provider/软件/桌面视觉 `production_attestation`，再返回同源 PRD production evidence coverage、`artifact_files` 与刷新后的 PRD readiness。示例 schema 见 `docs/examples/production-evidence-bundle.example.json`；该示例可用于 dry-run schema/coverage 校验，`cargo run -p pool-core --example import_production_evidence_bundle -- target/production-evidence-import-smoke` 可生成本地 artifact/metadata fixture 并 smoke 完整导入路径。真正导入前必须把 Provider/software/desktop production attestation、Provider artifacts/metadata、software artifacts、desktop trace 和 desktop vision artifacts 替换为已存在的本地文件。只有真实厂商 worker、真实软件插件/API/CLI 或真实视觉 controller 产出的证据才应使用这个入口；本地 mock/dry-run desktop trace 不能导入为生产视觉模型证据。

`submit-production-evidence-item <item.json>` 会调用 `POST /api/production-evidence/items`，适合外部 Provider worker、软件 operator 或桌面视觉 controller 完成单个任务后立即回填。`item.json` 是一个 JSON 对象，必须包含 `kind:"provider" | "software_action" | "desktop_vision"` 和对应的 `provider`、`software_action` 或 `desktop_vision` 对象；Provider、software 和 desktop vision item 还必须包含顶层 `production_attestation` 或 `evidence_json.production_attestation`。如果没有 `project_slug`，CLI 会从 `--project` 注入。后端会把单项 item 包装进同一套生产证据导入校验，继续拒绝占位 external id/attestation、远程 Provider artifact/metadata URL、本地 mock desktop trace 和缺失的本地 Provider/software/desktop 文件。完整批量收口仍推荐先 `validate-production-evidence` 再 `import-production-evidence`。

`provider-request-metadata` 会调用 `GET /api/provider-requests/metadata`，按已登记的 `provider_requests.id` 读取本地 request metadata/handoff JSON，不接受任意文件路径。它用于让 Agent/Hermes/gateway 审查等待审批的高成本 Provider 请求包。

`adapter-health` 会调用 `POST /api/adapter-health`，可一次性巡检 Provider 与软件矩阵，也可用 `--providers-only`、`--software-only`、`--no-providers`、`--no-software` 限定范围。

`software-health` 会调用 `POST /api/software-health`，只检查 Unreal、Blender、Resolve、TouchDesigner、Hermes 等 adapter 配置，不创建 task，也不写入 `software_actions`。

`run-software` 会调用 `POST /api/software-actions`，把软件控制动作写入 runtime。`--priority ApiMcp` 走 Unreal/Hermes MCP，`--priority SkillsCli` 走受控 CLI，`--priority DesktopRecognition` 生成桌面识别请求，`--requires-confirmation` 会让任务进入人工确认。`--evidence-json` 会随请求写入 `software_actions.command.payload_json.evidence`；只有真实软件插件/API/CLI 侧执行时才应追加 `--production-software`。

`run-workflow` 会调用 `POST /api/workflow-runs`，执行默认内容爆发闭环：默认蓝图、本地项目包、Agent/Hermes 决策、3DGS、Unreal 和三类输出 manifest。

`output-packages` 会调用 `GET /api/output-packages`，读取当前项目三类 deliverable catalog；它只查询 ready/missing 状态、本地 manifest 路径、预览合同和控制路由，不创建 task。

`output-package` 会调用 `POST /api/output-packages`，生成视频时间线、游戏构建、交互艺术 cue graph 三类本地 deliverable manifest，并同步写入 `assets`、`tasks` 和 `workflow_events`。

`output-result <target>` 会调用 `POST /api/output-packages/results`，把后段软件执行结果回填进本地 manifest 的 `execution_result` / `execution_history`，并写入 `output-package-result` task 与事件。`target` 为 `video`、`game` 或 `interactive_art`；常用参数包括 `--status`、`--runtime`、`--adapter-id`、`--software-action-id`、`--artifact`、`--metric label=value` 和 `--verification-json`。

`handoff-package` 会调用 `POST /api/handoff-packages`，把 runtime handoff runbook、运行前检查、runtime graph、`7-integration-readiness.json`、`8-runtime-handoff-package-manifest.json`、`5-worker-self-checks.sh`、`6-worker-self-checks-preflight.json` 和可选 snapshot 写入本地 `output/control/handoff/`，并同步写入 `assets`、`tasks` 和 `workflow_events`。接管操作者可先读 `8-runtime-handoff-package-manifest.json` 的 `read_order`、`operator_checklist`、`agent_entrypoint` 和 `mcp_resources`，再运行 `5-worker-self-checks.sh`，确认 Provider gateway、SDK worker、Unreal、Hermes 和通用软件 bridge 的本地 smoke 都可执行，再继续审批、重试或外部软件接管。

`agent-session` 会调用 `POST /api/agent-sessions`。`agent-session hermes` 可 staging 或执行 Hermes HTTP 指令；`agent-session agent-cli` 可 staging 或显式 `--execute` 受控执行 allowlist 命令。带 `--execute` 的会话如果先进入审批，会把 `execution_request` 写入 transcript，之后 `approve-task` 或 `retry-task` 可用同一个 task 恢复 Hermes HTTP / Agent CLI 执行；不带 `--execute` 的 staging 会话审批后只释放到 `ready`。`agent-transcript <session-id>` 会调用 `GET /api/agent-sessions/transcript`，按已登记的 session id 读取本地 transcript JSON，不接受任意文件路径。`agent-stream <session-id>` 会调用 `GET /api/agent-sessions/stream`，读取该 session 的 `agent-transcript` 和相关 `runtime-event` SSE 片段，可用 `--after-id` / `--last-event-id` / `--limit` 做增量抓取；Web/Hermes 控制台可优先使用 `/api/agent-sessions/ws?session_id=...` WebSocket 长连接。

`desktop-requests` 会读取 `GET /api/desktop-recognition/requests`，供外部桌面识别 controller 领取待处理请求。`desktop-run-next` 会调用 `POST /api/desktop-recognition/run-next`，用 runtime 内置 dry-run controller 推进前 N 个请求并回填结构化 evidence，用于验证 controller 协议、请求 metadata 透传和 task 状态更新；它不执行真实屏幕识别、点击或热键。需要执行明确坐标、快捷键或文本输入时，使用 `run_desktop_recognition_controller --mode=applescript` 读取同一队列并通过 macOS System Events 回填结果；若外部 OCR/视觉进程已写入 trace JSON，可加 `--vision-trace=<path>` 把 `visual_targets` 解析成点击坐标。需要让外部视觉/OCR 服务生成 trace 时，使用 `run_desktop_recognition_controller --mode=vision-http --vision-endpoint=<url> --vision-trace-output=<path> --evidence-bundle=<bundle.json>`；该模式只在 endpoint 成功返回、trace 写入本地后回填 `external_visual_model:true`，并可写出 `desktop_vision[]` production evidence bundle。`desktop-result` 会调用 `POST /api/desktop-recognition/results`，把识别、点击、热键或人工接管结果回填到 `software_actions.verification_json` 和关联 task。

`serve-mcp` 会启动 newline-delimited JSON-RPC stdio server，按 MCP `2025-11-25` stdio transport 约定通过 `stdin/stdout` 收发单行 JSON-RPC 消息（参考 https://modelcontextprotocol.io/specification/2025-11-25/basic/transports ），暴露：

- Resources：`resources/list` 与 `resources/read` 可读取 `pool://status`、`pool://tasks`、`pool://adapters`、`pool://integration-readiness`、`pool://provider-contracts`、`pool://provider-contracts/<provider-id>`、`pool://provider-gateway-worker`、`pool://software-contracts`、`pool://software-contracts/<adapter-id>`、`pool://unreal-mcp-bridge`、`pool://workflow/<workflow-id>`、`pool://runtime-graph`、`pool://runtime-budget`、`pool://runtime-preflight`、`pool://runtime-execution-plan`、`pool://runtime-handoff`、`pool://prd-readiness`、`pool://prd-completion-gate`、`pool://production-evidence-requirements`、`pool://production-evidence-tasks`、`pool://production-evidence-run-plan`、`pool://production-evidence-handoff`、`pool://production-evidence-item-template`、`pool://production-evidence-item-template/<task-id>`、`pool://node-context/<id>`、`pool://software-actions`、`pool://desktop-recognition` 等资源。
- Tools：`pool_status`、`pool_snapshot`、`pool_adapters`、`pool_integration_readiness`、`pool_software_contracts`、`pool_unreal_mcp_bridge`、`pool_runtime_graph`、`pool_runtime_budget`、`pool_runtime_preflight`、`pool_runtime_execution_plan`、`pool_runtime_execution_plan_run_next`、`pool_runtime_handoff`、`pool_prd_readiness`、`pool_prd_completion_gate`、`pool_prd_completion_package`、`pool_production_evidence_requirements`、`pool_production_evidence_tasks`、`pool_production_evidence_task_claim`、`pool_production_evidence_run_plan`、`pool_production_evidence_item_template`、`pool_production_evidence_item_from_ledger`、`pool_production_evidence_bundle_from_ledger`、`pool_production_evidence_handoff_package`、`pool_workflow_context`、`pool_node_context`、`pool_read_resource`、`pool_provider_gateway_worker`、`pool_provider_conformance_package`、`pool_integration_conformance_package`、`pool_agent_conformance_package`、`pool_worker_self_checks`、`pool_adapter_health`、`pool_provider_health`、`pool_run_provider`、`pool_validate_production_evidence`、`pool_merge_production_evidence`、`pool_closeout_production_evidence`、`pool_import_production_evidence`、`pool_validate_production_evidence_item`、`pool_submit_production_evidence_item`、`pool_provider_request_metadata`、`pool_software_health`、`pool_run_software`、`pool_run_node`、`pool_run_workflow`、`pool_output_package`、`pool_output_result`、`pool_handoff_package`、`pool_agent_session`、`pool_agent_transcript`、`pool_agent_stream`、`pool_desktop_requests`、`pool_desktop_run_next`、`pool_desktop_result`、`pool_approve_task`、`pool_cancel_task`、`pool_retry_task`。
- Prompts：`pool_content_burst_runbook`、`pool_3dgs_conversion_review`、`pool_software_handoff`、`pool_desktop_takeover`，用于让外部 Agent 先按标准流程读取资源、检查状态、再调用 tools。

这些 prompt 定义来自 `shared-core`，同一份 registry 也通过 Runtime HTTP `/api/prompts` 和 `/api/discovery` 的 `mcp_prompts` 字段暴露给不走 stdio MCP 的 Agent/Hermes/controller。Runtime HTTP `/api/discovery` 的 `mcp_tools` 字段同步暴露常用 MCP tool manifest，包括 `pool_worker_self_checks`、`pool_handoff_package`、`pool_run_provider`、`pool_run_software` 和 production evidence closeout tools，供 Web/Hermes 在建立 stdio 会话前完成工具选择。

`serve-mcp` 只复用 Runtime HTTP handler，不新建状态源。为了降低默认风险，MCP tools 不暴露 API Key 写入；凭证仍通过 `set-api-key`、Runtime HTTP `/api/api-keys` 或本地环境变量写入。

## Smoke

先初始化一份本地 runtime：

```bash
cargo run -p pool-core --example serve_runtime_http -- target/pool-cli-smoke/pool-runtime.sqlite once
```

读取 runtime：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo status
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo tasks
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo events --limit 24
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo adapters
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo integration-readiness
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-contracts triposplat
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo software-contracts unreal
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo unreal-mcp-bridge
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo workflow-context
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo node-context
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-budget
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-preflight
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-execution-plan
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-run-next
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo runtime-handoff
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo prd-readiness
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo production-evidence-requirements
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo handoff-package --node-id agent --output-dir worlds/demo/output --include-snapshot
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://tasks
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://runtime-execution-plan
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://prd-readiness
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://production-evidence-requirements
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://production-evidence-item-template
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://production-evidence-item-template/provider:midjourney:production_upstream
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://provider-contracts/midjourney
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://software-contracts/unreal
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo mcp pool://unreal-mcp-bridge
```

统一 worker 自检：

```bash
cargo run -p pool-cli -- worker-self-checks \
  --output-root target/pool-worker-self-checks \
  --software-adapter resolve
```

Unreal bridge worker 是常驻服务，单独开终端运行：

```bash
cargo run -p pool-cli -- unreal-mcp-bridge-worker \
  --bind 127.0.0.1:8790 \
  --output-root target/unreal-mcp-bridge-worker
```

Hermes bridge worker 同样是常驻服务：

```bash
cargo run -p pool-cli -- hermes-mcp-bridge-worker \
  --bind 127.0.0.1:8792 \
  --output-root target/hermes-mcp-bridge-worker
```

MCP stdio：

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  '{"jsonrpc":"2.0","id":3,"method":"prompts/list","params":{}}' \
  '{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"pool://status"}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp
```

MCP prompt：

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":5,"method":"prompts/get","params":{"name":"pool_software_handoff","arguments":{"project_slug":"demo","adapter_id":"blender","action_kind":"ExecuteCli"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"pool_provider_request_metadata","arguments":{"provider_request_id":"<provider-request-id>"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"pool_software_contracts","arguments":{"adapter_id":"unreal"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp
```

MCP Agent transcript：

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"pool_agent_transcript","arguments":{"session_id":"<agent-session-id>"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp

printf '%s\n' \
  '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"pool_agent_stream","arguments":{"session_id":"<agent-session-id>","limit":24}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp
```

MCP worker 自检：

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"pool_worker_self_checks","arguments":{"output_root":"target/pool-worker-self-checks-mcp","software_adapter":"resolve"}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp
```

MCP tool 写操作：

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"pool_run_software","arguments":{"adapter_id":"blender","action_kind":"ExecuteCli","priority":"SkillsCli","task_title":"MCP Blender CLI smoke","payload_json":{"command":"/bin/echo mcp-blender-ok","allowed_commands":["/bin/echo","echo"],"timeout_ms":2000,"max_output_bytes":1024}}}}' \
  | cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo serve-mcp
```

Provider/API Key 接入：

```bash
OPENAI_API_KEY=sk-test-local cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo set-api-key openai-image-2 \
  --api-key-env OPENAI_API_KEY \
  --metadata owner=local-smoke

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo api-keys

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-health world-labs-marble \
  --execution-mode mock

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-provider world-labs-marble \
  --execution-mode mock \
  --no-approval \
  --prompt "CLI provider smoke 3DGS" \
  --output-dir worlds/demo/output

# 另开一个终端先启动本地 gateway contract server：
# cargo run -p pool-core --example provider_gateway_mock_server -- --bind=127.0.0.1:8787
cargo run -p pool-cli -- provider-gateway-worker \
  --bind 127.0.0.1:8788 \
  --upstream http://127.0.0.1:8787

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-provider world-labs-marble \
  --execution-mode gateway \
  --endpoint http://127.0.0.1:8788 \
  --no-approval \
  --prompt "CLI gateway mock 3DGS" \
  --output-dir worlds/demo/output

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-provider worldlabs-marble \
  --execution-mode gateway \
  --endpoint http://127.0.0.1:9788 \
  --no-approval \
  --prompt "CLI production gateway evidence" \
  --output-dir worlds/demo/output/provider-evidence/worldlabs-marble \
  --evidence-json '{"source":"agent-cli","evidence_mode":"configured_gateway","family":"3dgs","production_attestation":"real-vendor-sdk-worker-2026-06-17"}' \
  --production-upstream

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo production-evidence-template \
  target/pool-cli-smoke/production-evidence-template.json --output-root target/pool-cli-smoke

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo validate-production-evidence \
  docs/examples/production-evidence-bundle.example.json

# import-production-evidence 需要真实外部生产 bundle，且 Provider artifacts/metadata、software artifacts 与 desktop trace/artifacts 已经落成本地文件

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo provider-request-metadata <provider-request-id>
```

Adapter 与软件控制：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo adapter-health --software-only

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo software-health blender \
  --priority SkillsCli

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software blender \
  --action execute-cli \
  --priority SkillsCli \
  --title "Blender CLI smoke" \
  --payload-json '{"command":"/bin/echo blender-runtime-ok","allowed_commands":["/bin/echo","echo"],"timeout_ms":2000,"max_output_bytes":1024}'

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software blender \
  --action execute-cli \
  --priority SkillsCli \
  --title "Blender production software evidence" \
  --payload-json '{"command":"/path/to/real/blender-worker","allowed_commands":["/path/to/real/blender-worker"],"timeout_ms":30000,"max_output_bytes":4096}' \
  --evidence-json '{"source":"agent-cli","control_profile":"skills_cli","evidence_mode":"production_software"}' \
  --production-software
```

三类输出包：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo output-package \
  --node-id outputs \
  --title "CLI output package" \
  --source-asset worlds/demo/output/1-world.glb \
  --duration-ms 12000
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo output-result game \
  --status succeeded \
  --runtime Unreal \
  --adapter-id unreal \
  --artifact unreal://level/demo_content_burst \
  --metric fps=60
```

Runtime 接管包：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo handoff-package \
  --node-id agent \
  --title "CLI handoff package" \
  --output-dir worlds/demo/output \
  --include-snapshot
```

响应中的 `report.manifest_path` 会指向 `worlds/demo/output/control/handoff/8-runtime-handoff-package-manifest.json`，`report.integration_readiness_path` 会指向 `worlds/demo/output/control/handoff/7-integration-readiness.json`，并直接返回 `report.operator_checklist`、`report.agent_entrypoint` 和 `report.mcp_resources`，方便 Agent/Hermes 在离线接管包内同步读取文件顺序、Provider、软件 adapter 与 Agent 控制状态。

Agent/Hermes 会话：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-session hermes \
  --instruction "inspect Unreal import queue" \
  --allowed-tool unreal \
  --allowed-tool blender

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-session agent-cli \
  --command-id echo \
  --title "Agent CLI echo" \
  --command "/bin/echo pool-agent-ok" \
  --tool cli \
  --execute \
  --allowed-command /bin/echo \
  --allowed-command echo \
  --timeout-ms 2000

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-transcript <agent-session-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo agent-stream <agent-session-id> --limit 24
```

桌面识别接管：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-software touchdesigner \
  --action run-viewport \
  --priority DesktopRecognition \
  --title "TouchDesigner desktop cue" \
  --payload-json '{"instruction":"find TouchDesigner perform mode and trigger cue 1","target_window":"TouchDesigner","visual_targets":["Perform","Cue 1","Output"]}'

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-requests

cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo desktop-result <software-action-id> \
  --status succeeded \
  --message "desktop controller finished" \
  --artifact worlds/demo/output/control/desktop-recognition/trace.json \
  --result-json '{"controller":"desktop-vision"}'
```

节点执行：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-node <3dgs-node-id> \
  --execution-mode mock \
  --prompt "CLI smoke 3DGS run"
```

完整内容爆发闭环：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-workflow \
  --title "CLI local content burst" \
  --prompt "run creative input to 3DGS to Unreal to outputs" \
  --source-input worlds/demo/source/0-reference.png \
  --agent-mode stage \
  --three-dgs-mode mock \
  --unreal-mode mock \
  --duration-ms 12000
```

强制真实 gateway/MCP 路径：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo run-workflow \
  --title "CLI gateway content burst" \
  --prompt "run real adapters" \
  --source-input worlds/demo/source/0-reference.png \
  --agent-mode hermes_http \
  --hermes-endpoint http://127.0.0.1:3900/hermes \
  --three-dgs-mode gateway \
  --three-dgs-endpoint http://127.0.0.1:8787 \
  --unreal-mode unreal_mcp \
  --unreal-endpoint http://127.0.0.1:8788
```

高成本 3DGS 节点会按 runtime 规则进入 `waiting_approval`，并写入 `provider_requests`。等待审批时 runtime 也会在 output dir 写入 `.0-provider-approval__<provider>-request.json`，供 Agent/Hermes/gateway 在真正调用外部 Provider 前审查。后续可通过 `pool-cli approve-task`、`/api/tasks/approve` 或 Web 控制台人工确认放行。

任务队列接管：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo approve-task <task-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo cancel-task <task-id>
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo retry-task <task-id>
```

`approve-task` 会调用 `POST /api/tasks/approve`。如果该 task 有 `provider_requests` 账本，runtime 会用同一个 task 继续原 Provider run；如果最近的软件动作是因 `--requires-confirmation` 暂停，runtime 会用同一个 task 恢复执行原 `run-software` 动作。`retry-task` 会调用 `POST /api/tasks/retry`，对已有 Provider/software 账本的失败、可重试或已取消任务，会按原请求重跑；Provider retry 会追加新的 `provider_requests` attempt，并在 `request_json.attempt.retry_of_provider_request_id` 记录父记录，便于审计每次重试。

## Agent / Hermes 用法

AgentSessionRunner 可 staging 这样的命令模板：

```text
pool-cli --project demo node-context
pool-cli --project demo runtime-budget
pool-cli --project demo runtime-preflight
pool-cli --project demo runtime-execution-plan
pool-cli --project demo runtime-graph
pool-cli --project demo workflow-context
pool-cli --project demo run-workflow --agent-mode stage --three-dgs-mode mock --unreal-mode mock
pool-cli --project demo provider-health world-labs-marble --execution-mode mock
pool-cli --project demo run-provider world-labs-marble --execution-mode mock --no-approval --prompt "Agent CLI 3DGS smoke"
pool-cli --project demo adapter-health --software-only
pool-cli --project demo software-health blender --priority SkillsCli
pool-cli --project demo run-software blender --action execute-cli --priority SkillsCli --payload-json '{"command":"/bin/echo blender-ok","allowed_commands":["/bin/echo","echo"]}'
pool-cli --project demo output-package --node-id outputs --source-asset worlds/demo/output/1-world.glb
pool-cli --project demo agent-session hermes --instruction "inspect output handoff" --allowed-tool unreal
pool-cli --project demo desktop-requests
pool-cli --project demo desktop-result <software-action-id> --status succeeded --message "controller finished"
pool-cli --project demo serve-mcp
pool-cli --project demo mcp pool://tasks
pool-cli --project demo mcp pool://runtime-budget
pool-cli --project demo mcp pool://runtime-preflight
pool-cli --project demo mcp pool://runtime-execution-plan
pool-cli --project demo approve-task <task-id>
```

显式 `execute:true` 时，Agent CLI 执行器仍要求命令二进制命中 allowlist，默认 allowlist 包含 `pool-cli`。执行不会经过 shell，因此不支持管道、重定向或交互式 TUI。

MCP 客户端配置示例：

```json
{
  "mcpServers": {
    "pool-runtime": {
      "command": "cargo",
      "args": [
        "run",
        "-p",
        "pool-cli",
        "--",
        "--db",
        "target/pool-cli-smoke/pool-runtime.sqlite",
        "--project",
        "demo",
        "serve-mcp"
      ]
    }
  }
}
```
