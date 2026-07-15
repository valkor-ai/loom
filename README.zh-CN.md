<div align="center">
  <img src="./assets/headbar.png" alt="Loom" width="100%">
  <p><strong>面向 agentic software delivery 的 loop engineering。</strong></p>
  <p>一套开源交付 harness，帮助 Claude Code、Codex、OpenCode 等 coding agents 完成更大的软件任务，并保留过程状态。</p>
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
    <a href="#上下文路由">上下文路由</a>
    ·
    <a href="#相关工作">相关工作</a>
    ·
    <a href="#faq">FAQ</a>
  </p>
  <p>
    <a href="./LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/License-Apache--2.0-blue.svg"></a>
    <a href="https://discord.gg/Yr7UjwbYPC"><img alt="Discord" src="https://img.shields.io/badge/Discord-Join-5865F2?logo=discord&logoColor=white"></a>
    <img alt="Rust" src="https://img.shields.io/badge/Rust-MCP%20runtime-b7410e?logo=rust&logoColor=white">
    <img alt="Python" src="https://img.shields.io/badge/Python-algorithms-3776AB?logo=python&logoColor=white">
    <img alt="Status" src="https://img.shields.io/badge/status-open-brightgreen">
  </p>
  <p>⭐ 如果 Loom 对你有帮助，欢迎点个 Star，让更多开发者看到它。</p>
</div>

## 什么是 Loom？

Coding agents 写代码很快，但完整收尾并不稳定：容易丢上下文、跳过检查，最后留下一个不太好接手的状态。

Loom 是一套开源 harness，运行在你已经使用的 agent 旁边。它把任务推进成一条简单的循环：规划、构建、测试、修复、预览、交接。

Loom 会保存关键项目状态：需求、任务进度、测试结果、运行时事实和修复记录。会话中断后，下一次运行可以从已有进度继续。

当一个任务不是单条 prompt 能解决的改动，而是涉及功能开发、部署、review、修复、预览或需要交接证据时，就适合使用 Loom。

## News

- **即将推出：** V-SEFM，一个面向软件交付的验证模型，正在准备中。细节后续公布。
- **2026 年 7 月：** Loom 已将 MCP runtime 从 TypeScript 迁移到 Rust，让核心更小、更快。

## 为什么需要 Harness？

现在的 coding agents 很快就能生成网站和应用。更麻烦的是第一版之后的事：保留需求、检查结果、修复失败，以及在会话中断后继续推进。

稍微长一点的任务，常见问题很固定：

失败模式 | Loom 的应对
--- | ---
只完成一部分就宣布完成 | 任务有边界，必须写出明确结果，Loom 再路由下一步。
目标漂移 | 已确认的 scope 和架构决策会被保存，并在后续会话继续使用。
自我验证偏差 | Review 和 repair 是单独步骤，并保留对应证据。
重复加载上下文 | Agent 读取紧凑的项目和任务状态，而不是反复扫完整仓库。
交付交接缺口 | 报告、日志、预览和修复历史会留下来，方便人或下一个 agent 检查。

## 从 Demo 到交付

AI coding 让 demo 变得很便宜。一段 prompt 就能生成页面、原型，或者一个自己用的小工具。

但交付是另一回事。稍微复杂的任务，仍然需要对齐需求、做架构取舍、跑测试、准备运行环境、修问题、看预览，并留下别人能接手的状态。

Loom 关注的就是这段差距。它给现有 coding agent 加上一条交付循环和一个保存进度的地方，让任务能扛住检查失败、上下文压缩和会话中断。

目标很简单：少一点从头来过，少一点半成品 agent 输出，多一点能验证、能交接的结果。

能力 | 解决的问题
--- | ---
Stateful delivery protocol | 把一次性 coding session 变成可恢复的交付循环，并用 `.loom/` 状态、request refs、结果文件、review 记录、修复请求和交接证据承载过程。
Requirement intelligence | 把松散 prompt 转成已确认的范围、业务规则、生命周期覆盖、页面办理路径和验收细节，让规划、执行和 review 都必须承接。
架构与系统设计 | 将已确认的技术基线和仓库事实转成面向实现的边界、行为、数据归属、运行时职责、NFR 目标、ADR 和故障模式决策，并用紧凑 id 传递给规划、执行、评审和修复阶段。
API 契约 | 在确实属于当前范围时，明确结构化接口、请求与响应模型、校验、错误行为、集合接口策略和兼容性规则。前端任务、联调检查、运行时探测和部署都消费已接受的 API 契约，不再自行猜测路径或前缀。
按技术栈选择的实现指导 | 根据已确认的 Technical Baseline 和当前任务归属，只加载任务需要的语言、框架、持久化和前端 references。代码与框架 references 提供仓库适配、实现模式、验证要求和反模式约束，不加载无关技术栈，也不重新选择技术。
Engineering contracts | 将运行时、代码质量和任务归属决策作为结构化 contracts 传递，而不是依赖 agent 反复记住 prompt 提醒。
Production UI guidance | 将 UI 质量前置到生成端：通过 surface decision、场景 references、布局密度、style asset plan、token 期望、禁用内容规则和 desktop/mobile 证据约束页面交付。
Targeted context routing | 让 agent 按需读取 field groups、reference profiles、任务 contracts 和 repair context，避免反复读取大文件或整份 artifact。
Task-scoped execution | 把交付拆成有边界的任务，并携带 source refs、写入边界、验证意图、结果模板和 continuation rules。
验证与评审纪律 | 只有任务拥有对应验证证据时才加载语言/框架测试指导；使用 review references 检查规格符合性和实现质量；只有明确分配浏览器行为时才加入 Playwright 浏览器闭环。运行时失败保留为环境证据，不误报成代码缺陷。
Review and repair loop | 通过 review signals、TaskResult evidence、repair contracts、多目标 repair 队列和再次验证，把实现与验证分离。
Runtime and deploy readiness | 面向本地 Docker Compose 预览准备 topology-aware services、build contexts、环境规则、端口、health checks、日志和 repair boundaries。
Knowledge-guided clarification | 让团队把本地域文档注册成具名知识库，构建本地可检索索引，并在需求澄清时只按当前步骤读取匹配片段。
Multi-agent MCP protocol | 让 Codex、Claude Code、OpenCode 和后续支持 MCP 的 agents 运行同一套交付状态机。

这条链路背后的技术指导集中在 `plugins/shared/loom/references/tech/`：架构（`arch`）、API 设计（`api`）、语言与 SQL 实现（`code`）、后端和前端框架（`backend`、`frontend`）、评审（`review`）以及 Playwright 验证（`test/playwright`）。这些内容不会作为一整套大 skill 一次性加载，而是由 Loom 根据已接受的技术事实和任务归属生成任务级选择，再把选中的 references 传给对应的架构、规划、执行、评审或浏览器闭环 request。

## 上下文路由

整体上下文路径：

```text
Your coding agent / app
(Codex, Claude Code, OpenCode, future agents...)
        |
        | delivery goal . repo context . logs . tests . preview evidence
        v
+----------------------------------------------------------------------------+
| Loom  (项目本地交付状态；完整 artifacts 留在 .loom/)                       |
|----------------------------------------------------------------------------|
| Dynamic workflow router -> Request manifest -> Agent read plan              |
|                              |                                             |
|                              |- requestReadPlan     grouped required reads  |
|                              |- MCP field resources targeted retrieval      |
|                              |- write targets       authorized artifact I/O |
|                              `- action result       next tool + compact view |
|                                                                            |
| Task contracts . evidence windows . fullLogRef . review/repair/resume state |
+----------------------------------------------------------------------------+
        |
        | compact instruction + selected field groups + retrieval path
        v
Agent turn / LLM context
```

## 前置条件

- 本机已安装一种受支持的 coding agent：Codex、Claude Code 或 OpenCode
- 使用 `loom deploy` 时需要 Docker

## 快速开始

按你使用的 coding agent 安装 Loom。安装脚本会自动识别 OS 和 CPU，下载对应平台包，校验 release 包的 `.sha256` 资产，安装 Rust MCP server，携带受控 Python 算法运行时，写入 agent 的 MCP registration，刷新本地插件，并执行 `loom-setup doctor`。

Codex：

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent codex
```

Claude Code：

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent claude-code
```

OpenCode：

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent opencode
```

同一台机器安装全部受支持 agent：

```bash
curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh | bash -s -- --agent all
```

Windows PowerShell：

```powershell
Invoke-WebRequest https://github.com/valkor-ai/loom/releases/latest/download/install.ps1 -OutFile install.ps1
.\install.ps1 -Agent codex
.\install.ps1 -Agent claude-code
.\install.ps1 -Agent opencode
.\install.ps1 -Agent all
```

重复执行同一条安装命令就是升级。安装器会在安装 MCP runtime 前清理确认属于 Loom 的旧 CLI 插件产物；如果发现无法确认归属的文件，会停止并提示人工处理路径，而不是覆盖用户文件。

如果是在仓库本地做验证，请使用同一个安装脚本的本地构建模式：

```bash
./install.sh --agent codex --local-build
```

它会构建 Rust MCP server 和 setup 二进制，生成同样的 release package layout，然后通过 `loom-setup` 完成安装并执行 doctor。后续本地修 bug 后都应该走这条路径，这样安装器、包结构、MCP registration 和插件刷新会一起被验证。

安装或更新 agent 插件后，请在目标项目里打开一个新的 agent 会话，让新的 MCP registration 和插件文件重新加载。

如果只想验证安装是否正常、但还不想开始需求交付，请在 coding agent 里使用 Loom 命令：

```text
@loom status     # Codex
/loom status     # Claude Code 和 OpenCode
```

`status` 是只读命令。对于还没有使用过 Loom 的项目，返回 `STATE_NOT_INITIALIZED` 也属于正常的 smoke check 结果：这说明插件命令可用，并且没有启动任何交付流程。

正常使用时不需要手动初始化 `.loom/`。从 agent 发起交付，例如 `@loom build ...` 或 `/loom build ...`，会在需要时自动为当前项目初始化本地交付状态。

## 如何使用

Loom 的正常使用入口是 code agent 里的本地插件。Codex 使用 `@loom`，Claude Code 和 OpenCode 使用 `/loom`。Rust MCP server 由 agent 的 MCP registration 自动启动，用户不需要手动启动。

### 使用知识库

知识库是可选能力，适合在交付工作依赖产品规则、业务文档、设计规范、操作手册或其他本地参考资料时使用。

Loom 会把知识库当作需求澄清辅助，而不是把它当成需求本身。需求澄清时，Loom 会搜索已启用且已成功构建的知识库索引，只读取当前澄清步骤匹配到的片段，并把有用信息转成对用户可见的问题或确认点。

知识库命令应在当前项目的 coding agent 会话里执行。下面示例使用 Codex 的 `@loom`；在 Claude Code 和 OpenCode 中，把同样的子命令换成 `/loom`。

新增知识库：

```text
@loom knowledge add --name product-rules ~/Documents/product-rules
@loom knowledge build product-rules
```

`--name` 必填且必须全局唯一。一个知识库可以包含单个文件、多个文件、单个目录、多个目录，或文件与目录混合。当前支持的格式是 `.md`、`.txt`、`.json`、`.yaml`、`.yml`、`.pdf`、`.docx`。

更新已有知识库的路径集合：

```text
@loom knowledge update product-rules --add-path ~/Documents/new-rules.md
@loom knowledge update product-rules --remove-path ~/Documents/old-rules.md
@loom knowledge update product-rules --replace-paths ~/Documents/current-rules
@loom knowledge build product-rules
```

如果只是已注册路径里的文件内容发生变化，直接重新执行 `build`。只有知识库包含的路径集合发生变化时，才需要先执行 `update`。

恢复未完成的知识库语义构建：

```text
@loom knowledge resume product-rules
```

如果知识库构建还没发布就中断了，例如重新打开 coding agent 会话，或者多 pack 语义构建没有跑完，可以使用 `resume`。它不会重新构建知识库，而是找到下一包未完成的语义构建任务，让 agent 接着执行直到索引发布。

查看和管理已有知识库：

```text
@loom knowledge list
@loom knowledge status product-rules
@loom knowledge pending product-rules
@loom knowledge discard product-rules
```

临时停用或重新启用某个知识库：

```text
@loom knowledge disable product-rules
@loom knowledge enable product-rules
```

删除知识库注册和 Loom 本地索引：

```text
@loom knowledge remove product-rules
```

`remove` 不会删除你的原始文档，只会删除 Loom 对这个知识库的注册信息、待构建队列和已构建索引。

### 运行交付

在 coding agent 中使用对应的 Loom 命令入口启动：

Codex：

```text
@loom build a visitor registration system
@loom continue
@loom review
@loom deploy
```

Claude Code 和 OpenCode：

```text
/loom build a visitor registration system
/loom continue
/loom review
/loom deploy
```

不同 agent 的入口不同，但都会进入同一套 Loom MCP 交付协议。插件会把请求路由到 Loom tools，并按 MCP server 返回的结构化 next action 继续执行。

当你希望 Loom 安全恢复或推进当前交付时，优先使用 `continue`。例如重新打开 agent 会话、任务中断、某个 tool action 成功后 agent 没继续往下走，或者你不确定下一步是什么时，都应该先用 `continue`。

```text
@loom continue     # Codex
/loom continue     # Claude Code 和 OpenCode
```

Agent 插件会自动设置 Loom 所需的路由环境。正常使用时请走 agent 命令入口；Loom 的产品运行时是 `loom-setup` 安装的 MCP server。

## 工作方式

Loom 作为本地 MCP 交付状态机运行。Agent 不需要凭记忆决定完整流程；它向 Loom 获取下一步 request，只读取声明的字段，写入指定 artifact，提交给 Loom 校验，再由 Loom 持久化并路由下一步。

1. 从 `.loom/` 状态启动或恢复。
2. 澄清并确认范围，必要时读取已注册的本地知识库。
3. 建立交付基线：repository context、technical baseline、planning contract 和 architecture artifact。
4. 将 contracts 转成任务计划，明确任务归属、read groups、写入边界、验证意图和结果模板。
5. 由 agent 执行有边界的任务，并写入带证据的 TaskResult。
6. Loom 对提交的 artifact 做校验、归一、持久化，并决定下一步路由。
7. 通过结构化 review signals 进行评审，并按问题类型路由到代码修复、任务计划修复、架构修复或人工 review。
8. 当执行 `deploy` 时，基于 runtime facts、Compose topology、环境规则、日志和 repair boundaries 准备本地部署预览。
9. 后续会话或其他 agent 可以从已保存状态继续，不需要重新整理交付上下文。

## 了解更多

需求 | 命令或文件
--- | ---
检查 Loom 插件可用性 | Codex 使用 `@loom status`，Claude Code 和 OpenCode 使用 `/loom status`
安装或升级 Codex 插件 | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent codex`
安装或升级 Claude Code 插件 | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent claude-code`
安装或升级 OpenCode 插件 | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent opencode`
安装或升级全部受支持插件 | `curl -fsSL https://github.com/valkor-ai/loom/releases/latest/download/install.sh \| bash -s -- --agent all`
运行本地部署预览 | Codex 使用 `@loom deploy`，Claude Code 和 OpenCode 使用 `/loom deploy`

## FAQ

<details>
<summary>Loom 和 <code>CLAUDE.md</code>、<code>AGENTS.md</code>、<code>.cursorrules</code> 有什么不同？</summary>

这些文件适合作为入口，但很容易变成越来越大的 prompt。Loom 在它们之外增加了有状态交付路由、任务 artifacts、review 结果、修复请求、部署证据和 MCP tools。

</details>

<details>
<summary>如果交付过程中断了怎么办？</summary>

Loom 会把项目本地交付状态保存到 `.loom/`，包括上下文、任务计划、结果记录、review notes、修复请求和部署证据。重新打开 agent 会话后，在 Codex 中运行 `@loom continue`，或在 Claude Code 和 OpenCode 中运行 `/loom continue`，Loom 会基于已保存的交付状态路由下一步。

</details>

<details>
<summary>Loom 会部署到生产环境吗？</summary>

暂时不会，后续会添加生产环境部署能力。当前部署能力聚焦于本地 Docker Compose 预览、验证、日志和修复指导。

</details>

## 卸载 Loom

如果你需要从本机移除某个 agent 的 Loom 插件，请使用 `loom-setup`：

```bash
~/.loom/bin/loom-setup uninstall --agent codex
~/.loom/bin/loom-setup uninstall --agent claude-code
~/.loom/bin/loom-setup uninstall --agent opencode
```

如果需要移除本机全部 Loom agent 插件：

```bash
~/.loom/bin/loom-setup uninstall --all
```

如果需要删除 Loom 的用户级 runtime 数据，包括已安装 runtime 和用户级知识库索引：

```bash
~/.loom/bin/loom-setup purge
```

`uninstall` 会保留项目本地 `.loom/` 交付状态。`purge` 的范围更大，只应在你确认要移除本机 Loom 用户级 runtime 和索引时使用。

卸载插件后，请打开新的 agent 会话，让对应 agent 重新加载本地 command/plugin 状态。

## 相关工作

Loom 关注 coding-agent skills、agentic engineering workflows 和软件工程评测方向的相关工作：

- [Matt Pocock's Skills](https://github.com/mattpocock/skills) - 面向需求澄清、领域语言、调试、TDD 和交接纪律的实用 agent skills。
- [SWE-bench](https://github.com/SWE-bench/SWE-bench) - 用于评测 coding agents 的真实软件工程任务。

## 支持方

<img src="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/logo/Zhejiang_University_Logo.svg" alt="浙江大学" width="220"> <img src="https://zonodqioyxil6r3k.public.blob.vercel-storage.com/logo/University_College_London_logo.svg" alt="伦敦大学学院" width="220">

## Star History

<a href="https://www.star-history.com/?repos=valkor-ai%2Floom&type=date&legend=top-left">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=valkor-ai/loom&type=date&theme=dark&legend=top-left&sealed_token=ITFmYmSJKgFXBLyRXm7qY3vsrnmzfFHgrR_5OOlRxgV6s2K_LV830pKK_rnMumgB8aECRZFwLZbfPy0fRqfliQz1DwShMs_7Gw5N3dZ75Kog79874801wBiMbEZ9TrhspECsQzAO6Wja93DPEbM-G4WJggxWb_VmSZWEzBrMC1twHlm0CVWGY9Bw5xA4" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=valkor-ai/loom&type=date&legend=top-left&sealed_token=ITFmYmSJKgFXBLyRXm7qY3vsrnmzfFHgrR_5OOlRxgV6s2K_LV830pKK_rnMumgB8aECRZFwLZbfPy0fRqfliQz1DwShMs_7Gw5N3dZ75Kog79874801wBiMbEZ9TrhspECsQzAO6Wja93DPEbM-G4WJggxWb_VmSZWEzBrMC1twHlm0CVWGY9Bw5xA4" />
    <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=valkor-ai/loom&type=date&legend=top-left&sealed_token=ITFmYmSJKgFXBLyRXm7qY3vsrnmzfFHgrR_5OOlRxgV6s2K_LV830pKK_rnMumgB8aECRZFwLZbfPy0fRqfliQz1DwShMs_7Gw5N3dZ75Kog79874801wBiMbEZ9TrhspECsQzAO6Wja93DPEbM-G4WJggxWb_VmSZWEzBrMC1twHlm0CVWGY9Bw5xA4" />
  </picture>
</a>

## 许可证

Loom 基于 [Apache License 2.0](./LICENSE) 开源。
