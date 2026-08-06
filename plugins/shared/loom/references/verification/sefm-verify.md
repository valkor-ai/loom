## 1. 需求与真实意图一致性

验证产物是否完成用户真正想要的事情，而不是只满足字面描述。

检查：

- 明确需求是否全部覆盖
- 隐含业务规则是否保留
- 正常流程和异常流程是否都符合预期
- 是否出现“按钮存在但流程不可用”
- 是否增加了用户未要求的副作用

## 2. 水平越权约束

验证用户不能访问同角色其他用户的数据。

重点检查：

- `user_id`
- `resource_owner_id`
- URL 中的对象 ID
- 查询条件是否绑定当前用户
- 批量接口是否逐对象授权
- 导出、下载、详情接口是否做对象级校验

典型测试：

```text
User A 请求 User B 的 resource_id
Expected: 403 或 404
```

## 3. 垂直越权约束

验证低权限用户不能调用高权限操作。

必须同时验证：

- 前端按钮隐藏
- 后端接口拒绝
- 直接构造请求仍然失败
- Token 权限变更后立即生效
- 管理操作是否记录审计

不能把“前端没有按钮”当作权限验证。

## 4. 租户隔离约束

验证 SaaS 场景下不同 Workspace / Tenant 的数据绝对隔离。

检查：

- 所有查询是否包含 tenant/workspace 条件
- 缓存 key 是否带 tenant
- 文件路径是否隔离
- GraphTrace 是否跨租户泄漏
- 异步任务是否继承正确租户
- 导出和搜索是否限制在当前租户


## 5. 状态机合法性约束

验证业务状态不能发生非法跳转。

例如：

```text
queued → processing → completed
queued → cancelled
processing → failed
failed → retrying
```

禁止：

```text
deleted → completed
refunded → shipping
closed → processing
```

每个状态转移都应有：

- 当前状态
- 操作
- 操作者
- 前置条件
- 目标状态
- 失败行为

## 6. 幂等性约束

验证重复请求、重试和消息重复投递不会产生重复副作用。

重点对象：

- 支付
- 创建订单
- 文件导入
- Webhook
- 任务提交
- Token 生成
- 发布和部署
- GraphTrace 实体写入

验证方式：

```text
同一 idempotency_key 请求 N 次
Expected:
- 只产生一次业务副作用
- 返回相同结果或明确幂等响应
```

## 7. 并发一致性约束

验证多个请求同时执行时，系统不会违反业务不变量。

重点场景：

- 库存扣减
- 账户余额
- 风险状态更新
- 多人同时编辑
- 同一任务被多个 Worker 领取
- 同一 Webhook 被并发处理

需要测试：

- race condition
- lost update
- double write
- stale read
- lock timeout
- optimistic concurrency conflict

## 8. 事务边界约束

验证数据库、消息队列、缓存和外部服务之间不会产生不可恢复的不一致。

例如：

```text
数据库写入成功
消息发送失败
```

不能直接返回“全部成功”。

需要验证：

- 事务是否完整
- 失败是否回滚
- 是否有 outbox / retry / compensation
- 是否可以安全重放
- 是否会出现重复消费

## 9. 数据完整性与迁移约束

验证数据没有丢失、错位或语义变化。

检查：

- 字段完整性
- 类型和精度
- ID 稳定性
- 时间和时区
- 关联关系
- 删除语义
- 迁移前后数量
- 新旧版本兼容
- rollback 是否可用

尤其要验证：

```text
新增字段
删除字段
字段改名
状态值变化
数据拆表
历史数据回填
```

## 10. API / 事件契约兼容性

验证代码产物没有破坏调用方。

检查：

- HTTP method
- path
- request schema
- response schema
- status code
- nullable semantics
- pagination
- error format
- webhook event payload
- SDK compatibility

需要覆盖：

```text
旧客户端 → 新服务
新客户端 → 旧服务
重复事件
乱序事件
未知字段
缺失字段
```

## 11. 错误可见性与可恢复性

验证失败没有被吞掉，也没有伪装成成功。

检查：

- 异常是否被吞掉
- 错误码是否准确
- 用户是否看到可行动提示
- 失败任务是否可重试
- 部分成功是否明确
- 是否保留失败上下文
- 是否支持回滚或补偿

禁止：

```text
catch error
return empty_result
```

然后让调用方误以为成功。

## 12. 资源与安全边界约束

验证输入、文件、命令和网络访问没有越过安全边界。

检查：

- SQL injection
- command injection
- path traversal
- SSRF
- XSS
- CSRF
- CORS
- 文件上传类型和大小
- 密钥泄露
- 日志敏感信息
- Prompt injection
- Agent 工具调用范围

对于 Harness，还要检查：

```text
Agent 是否只能访问任务授权目录？
Agent 是否只能调用当前任务允许的工具？
Agent 是否能读取 secret？
Agent 是否能执行危险命令？
```

## 13. 重试、超时和限流约束

验证外部服务异常时不会产生级联故障。

检查：

- timeout 是否存在
- retry 是否有上限
- retry 是否有 exponential backoff
- 是否识别 429
- 是否存在 circuit breaker
- 是否有 dead-letter queue
- 是否能避免 retry storm

尤其是 AI Harness：

```text
模型失败
→ 不应无限重试
→ 应记录失败原因
→ 应切换策略或安全停止
```

## 14. 可观察性与证据链约束

验证每个关键动作都能被追踪和复盘。

每次产物验证都要记录：

```json
{
  "check_id": "AUTH-OBJECT-001",
  "artifact_id": "build-2026-08-03",
  "input": "user_a requests resource_b",
  "expected": "403",
  "observed": "403",
  "status": "pass",
  "evidence": "api-test-119",
  "timestamp": "..."
}
```

## Loom 本地验证输出合同

Loom 会在验证请求的 `subject.checkPlan` 中生成唯一的 `checkPlan`。Code Agent 只能填写其中 `applicability=required` 的检查项；`not_applicable` 项由 Loom 根据已接受的结构化交付事实确定，Agent 不得自行补充或覆盖。`prompt` 只提供本验证规则文档的引用，不包含第二份检查计划。

### 检查范围与结论判定

每个检查项只能报告自己在 `subject.checkPlan` 中声明的范围：

- `AUTH-HORIZONTAL`、`AUTH-VERTICAL` 负责身份、对象所有权和服务端权限。
- `SECURITY-BOUNDARY` 负责输入、密钥、路径、命令和网络访问边界，不重复报告认证授权结论。
- `OBSERVABILITY-EVIDENCE` 负责请求、写操作和响应的追踪证据。

`unknown` 只表示当前证据无法判断通过或失败。没有专项测试不等于缺陷不存在；如果源代码或运行结果已经确认违反规则，必须填写 `fail`。不要把已确认的失败放入 `unknown_checks`，也不要把一个检查项的发现复制到另一个检查项。

Agent 提交的外层 `status` 是候选值。Loom 接受结构合法的结果后，根据 required 检查项的状态重新计算最终状态：硬阻断失败为 `blocked`，存在非阻断失败或 unknown 为 `inconclusive`，全部通过才为 `pass`。不要因为外层 status 与这些规则不一致而重复提交结果。

Code Agent 只写入以下字段；`artifact_id`、`verification_id`、范围、来源引用、checkPlan 和统计数量由 Loom 根据验证请求补充，Agent 不得自行创建或修改：

```json
{
  "status": "pass | blocked | inconclusive",
  "checks": [
    {
      "check_id": "AUTH-HORIZONTAL",
      "category": "AUTHORIZATION",
      "rule": "HORIZONTAL_PRIVILEGE",
      "status": "pass | fail | unknown",
      "input": "User A requests User B resource",
      "expected": "403 or 404",
      "observed": "403",
      "evidence": "api-test-119",
      "timestamp": "2026-08-05T00:00:00Z"
    }
  ],
  "blocking_failures": [
    {
      "finding_id": "finding-auth-001",
      "check_id": "AUTH-VERTICAL",
      "severity": "critical",
      "summary": "Write permission is controlled by client input.",
      "remediation": "Derive write permission from authenticated server-side identity and role data."
    }
  ],
  "warnings": [],
  "unknown_checks": [],
  "recommended_actions": []
}
```

`status` 是验证结果状态；单条检查的 `status` 是检查状态。`artifact_status` 不作为独立字段使用。水平越权、垂直越权、租户隔离、幂等性、状态机和事务边界任一硬约束失败时，外层 `status` 必须为 `blocked`。没有可复核证据时不得填写 `pass`，无法判断时填写 `unknown` 并说明原因。

每条 `checks` 必须覆盖 Loom `checkPlan` 中声明为 `required` 的检查项，不要输出 `not_applicable` 检查。`unknown_checks` 只能引用状态为 `unknown` 的检查，并说明无法判断的原因。`blocking_failures` 只能引用已有失败检查，每个独立根因使用唯一 `finding_id`，不得重复粘贴检查中的证据、期望值和观察值。

结果必须严格匹配请求中的 `resultSchema`：不得增加未声明字段，不得使用 `null` 替代必填字符串，也不得修改 MCP-owned 字段。提交合同错误时只修正返回的字段路径，不重新执行已经完成的验证。

## Loom 本地验证修复输出合同

Repair 是修复业务代码后重新执行 V-SEFM 的 Agent 任务，不是反复修改验证规则或 `.loom` 产物。Repair request 中的 `repairWriteBoundary` 是项目根目录范围和受保护路径说明；`subject.changedFiles` 只提供 scope hint，不是写入白名单。

Code Agent 只需要提交最小结果：

```json
{
  "status": "ready",
  "summary": "已修复阻断项。",
  "details": {
    "修复说明": "这里可以提供面向用户的说明。"
  }
}
```

`status` 只能是 `ready` 或 `blocked`，`summary` 必须是非空字符串。`details` 是面向用户和审计的透明内容，内部可以使用任意 JSON 结构。`verification_commands`、`changed_files`、`resolved_failure_refs` 和 `remaining_findings` 不属于 Agent 合同；Loom 会从执行记录、修复前后文件快照和重新验证结果中生成对应信息。

Repair Agent 可以修改项目根目录下为修复阻断项所需的普通业务代码、配置、测试、迁移、构建和部署文件，但不得修改 `.loom/**`、`.git/**`、V-SEFM 验证规则或 V-SEFM 配置。`ready` 只表示 Agent 声明修复完成，Loom 接收后必须重新执行同一验证计划，不能直接视为通过。

如果相同结果在相同合同错误下重复提交，这是 Loom/MCP 合同故障，不是软件质量结论。Loom 必须保存原始结果和错误指纹，停止重复提交并记录 `verification_unavailable`，不能把该故障路由为 `manual_review`。

必须能够追踪：

```text
请求
→ 权限判断
→ 数据查询
→ 状态变化
→ 外部调用
→ 最终响应
```

## 15. 回归与兼容性约束

验证新产物没有破坏原有能力。

检查：

- 原测试集
- 关键用户流程
- 历史 API
- 旧数据读取
- 旧权限模型
- 数据迁移
- 相关模块行为
- 多版本客户端

不能只测试新功能。

## 16. 性能与容量约束

验证产物在真实数据规模和并发下仍然可用。

检查：

- 查询复杂度
- N+1
- 全表扫描
- 内存峰值
- 大文件处理
- 大批量导入
- 并发请求
- 连接池
- 队列积压
- Agent token 和工具预算

## 建议的验证分类

可以按以下类型组织：

```text
BUSINESS
- requirement coverage
- user intent
- state transition

AUTHORIZATION
- horizontal privilege
- vertical privilege
- tenant isolation
- object-level access

CONSISTENCY
- idempotency
- concurrency
- transaction boundary
- event ordering

DATA
- integrity
- migration
- schema compatibility

RELIABILITY
- retry
- timeout
- rate limit
- recovery

SECURITY
- injection
- secret exposure
- path traversal
- SSRF
- prompt injection

OPERABILITY
- observability
- auditability
- rollback
- evidence chain

QUALITY
- regression
- performance
- maintainability
```


其中越权、租户隔离、幂等、状态机和事务一致性应被视为**阻断交付的硬约束**：

```text
任意一项失败
→ status = blocked
→ 不允许标记为 verified
→ 必须进入 repair 或人工审查
```

## 最终验收结果建议

不要只输出 `pass / fail`，而应返回：

```json
{
  "artifact_id": "artifact_123",
  "status": "blocked",
  "blocking_failures": [
    {
      "finding_id": "finding-tenant-001",
      "check_id": "TENANT-ISOLATION",
      "severity": "critical",
      "summary": "Tenant B data is visible to tenant A.",
      "remediation": "Add tenant identity predicates to resource lookup and verify cross-tenant access."
    }
  ],
  "passed_checks": 42,
  "warning_count": 5,
  "unknown_count": 2,
  "recommended_actions": [
    "Add workspace_id predicate to project lookup",
    "Add cross-tenant authorization integration test"
  ]
}
```
