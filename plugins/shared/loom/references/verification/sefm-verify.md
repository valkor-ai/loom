# V-SEFM Verification Rules

本文档是 V-SEFM 本地验证的唯一规则来源。每个章节定义一个验证规则的范围和证据要求；规则目录提供对应的机器可读 `check_id` 与阻断属性。

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

认证证据必须来自服务端已经验证的 session、token 或身份提供方上下文。客户端自行提交的身份请求头、表单值、查询参数、resource owner id、角色或用户 id 都不能作为认证证据；验证时必须检查服务端如何建立并校验当前操作者身份。

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

每次产物验证都应记录可复核的输入、预期、实际结果、证据和时间。

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

## 17. 浏览器质量约束

验证已声明的 Playwright 浏览器流程能够在项目真实运行环境中完成。

检查：

- Playwright 配置、浏览器项目和测试入口是否存在
- 项目声明的关键浏览器流程是否实际执行
- 桌面和窄屏视口是否按项目声明完成验证
- 测试结果是否包含可复核的命令、结果和失败原因
- 浏览器依赖或系统动态库缺失时，必须记录环境阻断，不得伪造通过

<!-- loom-rule-catalog:start -->
{
  "schemaVersion": "1.0",
  "rules": [
    { "id": "BUSINESS-INTENT", "section": "1", "blocking": false },
    { "id": "AUTH-HORIZONTAL", "section": "2", "blocking": true },
    { "id": "AUTH-VERTICAL", "section": "3", "blocking": true },
    { "id": "TENANT-ISOLATION", "section": "4", "blocking": true },
    { "id": "STATE-MACHINE", "section": "5", "blocking": true },
    { "id": "IDEMPOTENCY", "section": "6", "blocking": true },
    { "id": "CONCURRENCY", "section": "7", "blocking": false },
    { "id": "TRANSACTION", "section": "8", "blocking": true },
    { "id": "DATA-INTEGRITY", "section": "9", "blocking": false },
    { "id": "API-COMPATIBILITY", "section": "10", "blocking": false },
    { "id": "ERROR-RECOVERY", "section": "11", "blocking": false },
    { "id": "SECURITY-BOUNDARY", "section": "12", "blocking": false },
    { "id": "RETRY-TIMEOUT-RATE-LIMIT", "section": "13", "blocking": false },
    { "id": "OBSERVABILITY-EVIDENCE", "section": "14", "blocking": false },
    { "id": "REGRESSION-COMPATIBILITY", "section": "15", "blocking": false },
    { "id": "PERFORMANCE-CAPACITY", "section": "16", "blocking": false },
    { "id": "BROWSER-QUALITY", "section": "17", "blocking": false }
  ]
}
<!-- loom-rule-catalog:end -->
