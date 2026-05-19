# disable_keywords 配置说明

## 目的
在 *thinking* 模式下，当上游返回错误信息包含特定关键字时，系统会冻结对应的通道 6 小时，以防止持续错误请求。

## 默认关键字集合
如果用户在 **Settings → Config** 中没有手动配置 `disable_keywords`，系统会使用以下默认关键字列表（与数据库首次创建时的默认值保持一致）：

```
Your credit balance is too low
This organization has been disabled.
You exceeded your current quota
Permission denied
The security token included in the request is invalid
Operation not allowed
Your account is not authorized
insufficient_quota
quota_exceeded_error
token plan limit exhausted
Upstream rate limit exceeded
invalid api key
Unauthorized - Invalid token
```

## 自定义关键字
- 前往 **Settings → Config** 页面，编辑 **disable_keywords** 文本框。
- 每行填写一个关键字，匹配时不区分大小写。
- 保存后立即生效，覆盖默认集合。

## 行为说明
- 当 `disable_keywords` 为空或仅包含空行时，系统自动回退到 **默认关键字集合**。
- 关键字匹配通过 `should_disable_entry_for_message` 实现，匹配任意关键字即触发冻结。
- 冻结时长固定为 6 小时（可在代码中修改），冻结后会记录日志 `Freezing channel … because entry … matched upstream error keyword`。

## 关联文档
- `src-tauri/src/database/schema.rs`：首次创建数据库时写入的默认关键字。
- `src-tauri/src/proxy/forwarder.rs`：实际读取并回退到默认关键字的实现。
