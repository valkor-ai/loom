<div align="center">
  <img src="./assets/loom-logo.svg" alt="Loom" width="560">
  <p><strong>面向 agentic software delivery 的 loop engineering。</strong></p>
  <p>一套开源交付 harness，把 Claude Code、Codex、OpenCode 等 coding agents 变成可重复的软件交付系统。</p>
  <p>
    <a href="./README.md">English</a>
    ·
    <a href="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/Loomline-v0.pdf">技术报告</a>
    ·
    <a href="./docs/use-cases.zh-CN.md">使用场景</a>
    ·
    <a href="#快速开始">快速开始</a>
    ·
    <a href="#如何使用">如何使用</a>
    ·
    <a href="#省-token-的上下文方案">省 Token</a>
    ·
    <a href="#相关工作">相关工作</a>
    ·
    <a href="#faq">FAQ</a>
  </p>
  <p>
    <a href="./LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/License-Apache--2.0-blue.svg"></a>
    <a href="https://discord.gg/Yr7UjwbYPC"><img alt="Discord" src="https://img.shields.io/badge/Discord-Join-5865F2?logo=discord&logoColor=white"></a>
    <img alt="Node.js" src="https://img.shields.io/badge/Node.js-20%2B-339933?logo=node.js&logoColor=white">
    <img alt="Status" src="https://img.shields.io/badge/status-open-brightgreen">
  </p>
</div>

## 什么是 Loom？

Loom 是一套面向现有 coding agents 的开源交付 harness。它不替代你正在使用的模型或编辑器，而是把每个交付目标变成一条结构化循环：规划、构建、验证、修复、预览和交接。

Loom 使用 dynamic workflows 为每个交付目标选择合适路径，并让这条路径变得持久：项目上下文、任务 contracts、后端状态、测试结果、预览证据、修复记录和交接报告都会被保存下来，让下一次会话、另一个 agent 或 CLI 不需要从头开始。

Loom 不是一次性的 prompt chain，而是把交付变成一个 loop：路由下一步、执行、验证、记录证据、在需要时修复，并从已保存状态继续推进。

Coding agents 已经会写代码。Loom 帮助它们从 idea 到 release 都守住交付承诺，同时减少无效 token 消耗。

当一个需求不只是一次性改代码，而是需要澄清、架构、任务拆分、实现证据、review、修复、预览、部署或清晰交接时，就适合使用 Loom。

## 为什么需要 Harness？

生成网站和应用正在变成基础能力。更难的问题是可靠交付：agent 在 compaction 之后如何不偏离目标，长任务中如何保留需求，如何避免只相信自己的检查结果，如何修复失败，以及中断后如何从正确步骤恢复。

长时间 agent 工作常见的问题很固定：

失败模式 | Loom 的应对
--- | ---
只完成一部分就宣布完成 | 有边界的任务、明确结果文件、continue 路由和 final-response guard，避免 agent 在部分进展后提前收工。
目标漂移 | 已确认 scope、architecture contracts、task plans 和紧凑 context packs，把原始目标保留到多个会话之后。
自我验证偏差 | Review、验证、修复请求和证据记录，将实现与验证拆开。
Token 浪费 | 项目摘要、任务图、后端/运行时状态、测试结果和部署证据，减少反复读取全仓库。
交付交接缺口 | 交付报告、预览检查、日志和修复历史，让最终状态可以被人和其他 agent 检查。

真正难的是模型外围的 harness：持久状态、有边界的任务、路由、验证、恢复，以及人能读懂的交付证据。Loom 把 dynamic workflows 作为运行模式，再提升到项目级交付 harness，让交付过程可以跨越中断、compaction、adapter 切换和后续交接。

这也是 Loom 和 prompt 文件、一次性 workflow、单 agent 脚本的区别：它把交付状态写入 `.loom/`，通过 agent-neutral CLI 暴露流程，并把验证、修复、预览和交接变成协议里的一级步骤。

## 从 Demo 到交付

Vibe Coding 和 AI Coding 正在让越来越多的人具备软件构建能力。过去只有程序员和专业团队才能完成的事情，现在普通构建者也可以借助 Coding Agent 快速做出 Demo、产品原型，甚至开发自己日常使用的软件工具。

但从一个“能跑起来的 Demo”或“自己能用的小工具”，到一个真正可以被信任、可以交付、可以持续维护的生产级应用，中间仍然有一条巨大的鸿沟。

这条鸿沟不只是模型能力的问题。即使模型能力持续增强，构建者仍然需要处理很多交付层面的工作：澄清需求、保存项目上下文、做架构判断、拆分任务、准备后端和运行环境、执行测试、定位问题、修复错误、再次验证、预览结果、记录交付证据，以及为后续迭代保留清晰状态。

Loom 就是为弥合这条鸿沟而存在的。

它不是替代 Claude Code、Codex、Cursor Agent 或其他 Coding Agent，而是在这些工具之上增加一层开源的软件交付协议。Loom 帮助 Agent 从“一次性写代码”走向“可重复的软件交付”：先确认需求，再规划任务，持续保存上下文，执行检查，修复失败，重新验证，并最终报告交付证据。

我们的目标很简单：

**帮助构建者从 Vibe Coding 的 Demo 和自用工具，走向更可靠、更可维护、更接近生产级的软件产品，同时减少手工交付成本和无效 Token 消耗。**

能力 | 解决的问题
--- | ---
Dynamic workflows | 把每个交付目标变成一条可自适应的循环：澄清、规划、执行、验证、修复和交接。
Delivery harness | 把需求澄清、规划、构建、检查、预览、review、修复和报告变成稳定流程。
Requirement intelligence | 把需求澄清从普通聊天确认变成交付质量门：将已确认的阶段范围、业务规则、生命周期覆盖和页面办理路径沉淀为结构化上下文，让后续规划、编码和 review 必须承接。
Token-saving context | 沉淀项目摘要、任务图、后端/运行时状态、测试和部署结果，减少 agent 反复读取全仓库。
Task contracts | 将宽泛目标拆成有边界的任务，并带上 source refs、验收意图、结果文件和 continuation rules。
Executable tools | 提供上下文整理、任务路由、结果记录、部署检查和交付证据等 CLI 命令。
Backend readiness | 将数据库、Auth、Storage、Functions、环境变量、服务和运行时需求纳入交付状态。
UIX guidance | 将视觉方向、交互流程、响应式状态、可访问性期望和产品特定界面细节作为交付要求沉淀下来。
Verification loop | 把 smoke test、Playwright 类验证、日志、错误摘要、修复请求和再次验证串成闭环。
Multi-agent protocol | 让 Claude Code、Codex、OpenCode 等工具共享同一套交付流程。

## 省 Token 的上下文方案

整体上下文路径：

```text
Your coding agent / app
(Codex, Claude Code, OpenCode, future adapters...)
        |
        | delivery goal . repo context . logs . tests . preview evidence
        v
+----------------------------------------------------------------------------+
| Loom  (项目本地交付状态；完整 artifacts 留在 .loom/)                       |
|----------------------------------------------------------------------------|
| Dynamic workflow router -> Request manifest -> Agent read plan              |
|                              |                                             |
|                              |- .refs/*.json        full authority          |
|                              |- fieldGroups         grouped required reads  |
|                              |- inspect selectors   targeted retrieval      |
|                              `- compact envelope    next action + refs      |
|                                                                            |
| Task contracts . evidence windows . fullLogRef . review/repair/resume state |
+----------------------------------------------------------------------------+
        |
        | compact instruction + selected refs + retrieval path
        v
Agent turn / LLM context
```

最新 11-case agent-run benchmark 中，Codex + Loom 相比单独使用 Codex 节省了 15.8% token，同时保持 100% 完成度。当前数据对比可以查看 [最新 benchmark 结果](./benchmarks/agent-run/results/latest.md)，运行方式见 [run guide](./benchmarks/agent-run/README.md)。

## 前置条件

- Node.js >= 20
- npm
- 你要安装的 adapter 对应的 coding agent CLI：Codex 需要 Codex CLI，Claude Code 需要 Claude Code CLI，OpenCode 需要 OpenCode CLI
- 使用 `loom deploy` 时需要 Docker

## 快速开始

```bash
# 1. 克隆 Loom 并安装依赖
git clone https://github.com/valkor-ai/loom.git
cd loom
npm install

# 2. 安装或刷新你要使用的本地 adapter

# Codex：安装或更新 Codex 本地插件和共享 launcher。
npm run plugin:install-codex

# Claude Code：安装 Claude Code 插件包、skills、hooks 和 launcher。
npm run plugin:install-claude

# OpenCode：安装本地 slash commands、plugin hook、references 和 launcher。
npm run plugin:install-opencode

# 全部 adapters：适合在本机开发或测试多个 agents 时使用。
npm run plugin:install-adapters
```

每个 adapter 安装脚本都会构建 CLI，在 `~/.loom/bin/loom-cli` 写入稳定 launcher，在 `~/.loom/adapters/<agent>` 记录 adapter 元数据，并刷新对应 agent 的本地 adapter 文件。面向 agent 的命令会使用这个 launcher，不依赖用户 shell `PATH` 里的裸 `loom`。

`plugin:install-adapters` 会一次性安装或刷新 Codex、Claude Code 和 OpenCode。

安装或更新 adapter 后，请打开一个新的 agent 会话，让本地插件重新加载。

如果只想验证安装是否正常、但还不想开始需求交付，可以执行：

```bash
"$HOME/.loom/bin/loom-cli" --version
"$HOME/.loom/bin/loom-cli" status --project-root /path/to/project
```

`status` 是只读命令。对于还没有使用过 Loom 的项目，返回 `STATE_NOT_INITIALIZED` 也属于正常的 smoke check 结果：这说明 launcher 可用，并且没有启动任何交付流程。也可以在新的 agent 会话里验证 adapter 命令：Codex 使用 `@loom status`，Claude Code 和 opencode 使用 `/loom status`。

正常使用时不需要手动先执行 `loom init`。从 agent 发起交付，例如 `@loom build ...` 或 `/loom build ...`，会在需要时自动为当前项目初始化 `.loom/`。如果你是直接使用 CLI，也可以显式初始化：

```bash
"$HOME/.loom/bin/loom-cli" init --project-root /path/to/project
```

## 如何使用

在 coding agent 中使用对应的 Loom 命令入口启动：

Codex：

```text
@loom build a visitor registration system
@loom plan this feature first
@loom continue
@loom review
@loom deploy
```

Claude Code 和 OpenCode：

```text
/loom build a visitor registration system
/loom plan this feature first
/loom continue
/loom review
/loom deploy
```

不同 adapter 的入口不同，但都会进入同一套 Loom 交付协议。adapter 会自动设置自己的 agent profile，并使用 adapter 安装脚本写入的共享 launcher。

当你希望 Loom 安全恢复或推进当前交付时，优先使用 `continue`。例如重新打开 agent 会话、任务中断、某个命令成功后 agent 没继续往下走，或者你不确定下一步内部流程是什么时，都应该先用 `continue`。不要先手动猜 `next-task`、`review`、`repair` 这类内部命令；先运行 `continue`，再按返回的 instruction 执行。

```text
@loom continue     # Codex
/loom continue     # Claude Code 和 OpenCode
```

也可以通过稳定 launcher 直接运行 CLI：

```bash
"$HOME/.loom/bin/loom-cli" status --project-root /path/to/project
"$HOME/.loom/bin/loom-cli" plan --project-root /path/to/project --request "Add team invitations"
"$HOME/.loom/bin/loom-cli" continue --project-root /path/to/project
"$HOME/.loom/bin/loom-cli" review --project-root /path/to/project
"$HOME/.loom/bin/loom-cli" deploy run --project-root /path/to/project
```

Agent adapter 通常会自动设置 `LOOM_AGENT_PROFILE` 和 `LOOM_COMPACT_OUTPUT`。如果你正在接入新的 adapter，路由命令应通过 launcher 执行，并建议使用 compact output：

```bash
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" continue --project-root /path/to/project
```

## 工作方式

Loom 在项目本地创建 `.loom/` 交付状态，并把它作为 agent 下一步行动的 source of truth。核心循环很短：

1. 捕获并确认交付范围。
2. 生成紧凑 context pack。
3. 生成 planning、architecture 和 task contracts。
4. 每次执行一个有边界的任务。
5. 记录证据并运行验证。
6. Review、修复、再次检查。
7. 报告最终交付状态。

## 了解更多

需求 | 命令或文件
--- | ---
查看可用命令 | `"$HOME/.loom/bin/loom-cli" --help`
安装或刷新全部 adapters | `npm run plugin:install-adapters`
安装或刷新 Codex adapter | `npm run plugin:install-codex`
安装或刷新 Claude Code adapter | `npm run plugin:install-claude`
安装或刷新 OpenCode adapter | `npm run plugin:install-opencode`
运行本地部署预览 | `"$HOME/.loom/bin/loom-cli" deploy run --project-root /path/to/project`

## FAQ

<details>
<summary>Loom 和 <code>CLAUDE.md</code>、<code>AGENTS.md</code>、<code>.cursorrules</code> 有什么不同？</summary>

这些文件适合作为入口，但很容易变成越来越大的 prompt。Loom 在它们之外增加了有状态交付路由、任务 artifacts、review 结果、修复请求、部署证据和 agent-neutral CLI 命令。

</details>

<details>
<summary>如果交付过程中断了怎么办？</summary>

Loom 会把项目本地交付状态保存到 `.loom/`，包括上下文、任务计划、结果记录、review notes、修复请求和部署证据。重新打开 agent 会话后，在 Codex 中运行 `@loom continue`，或在 Claude Code 和 OpenCode 中运行 `/loom continue`，Loom 会基于已保存的交付状态路由下一步。

</details>

<details>
<summary>Loom 会部署到生产环境吗？</summary>

暂时不会，后续会添加生产环境部署能力。当前部署能力聚焦于本地 Docker Compose 预览、验证、日志和修复指导。

</details>

## 卸载本地 Adapter

如果你需要从本机移除某个本地 adapter，可以使用对应的卸载命令：

```bash
npm run plugin:uninstall-codex
npm run plugin:uninstall-claude
npm run plugin:uninstall-opencode
```

如果需要移除本机全部 Loom adapters：

```bash
npm run plugin:uninstall-adapters
```

卸载脚本只会删除用户级 adapter 安装产物，例如 Codex plugin source/cache entry、Claude Code commands/skills、OpenCode commands/plugins/references，以及 `~/.loom/adapters/<agent>` 元数据。它不会删除任何项目目录里的 `.loom/` 交付状态。只有当 `~/.loom/adapters/` 下已经没有其他 Loom adapter 元数据时，共享 launcher `~/.loom/bin/loom-cli` 才会被删除。

卸载 adapter 后，请打开新的 agent 会话，让对应 agent 重新加载本地 command/plugin 状态。

## 相关工作

Loom 关注 coding-agent skills、agentic engineering workflows 和软件工程评测方向的相关工作：

- [Matt Pocock's Skills](https://github.com/mattpocock/skills) - 面向需求澄清、领域语言、调试、TDD 和交接纪律的实用 agent skills。
- [SWE-bench](https://github.com/SWE-bench/SWE-bench) - 用于评测 coding agents 的真实软件工程任务。

## 支持方

<img src="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/logo/Zhejiang_University_Logo.svg" alt="浙江大学" width="220"> <img src="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/logo/University_College_London_logo.svg" alt="伦敦大学学院" width="220">

## Star History

<a href="https://www.star-history.com/#valkor-ai/loom&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=valkor-ai/loom&type=Date&theme=dark" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=valkor-ai/loom&type=Date" />
    <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=valkor-ai/loom&type=Date" />
  </picture>
</a>

## 许可证

Loom 基于 [Apache License 2.0](./LICENSE) 开源。
