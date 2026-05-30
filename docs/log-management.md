# 日志管理 (Log Management)

## 概述
负责应用运行日志的展示、筛选及数据维护。

## 需求分析
- [x] 优化清理数据交互：将 `window.confirm` 替换为 UI 组件库的 `Dialog` 组件，提升用户体验。
- [x] 增加清理过程中的 Loading 状态，防止重复点击。
- [x] 调整日志列表中部分列的宽度（如 `Switch` 列），优化屏幕空间利用。

## 实现计划
- **前端 (React)**:
  - 引入 `Dialog`, `DialogContent`, `DialogHeader`, `DialogFooter` 组件。
  - 新增 `showClearDialog` 和 `isClearing` 状态管理。
  - 优化 `handleClearDetails` 函数，处理异步清理逻辑及异常捕获。
- **国际化 (i18n)**:
  - 新增 `log.clearConfirmTitle` 词条（中英文）。

## 开发进度
| 任务 | 状态 | 备注 |
|------|------|------|
| 清理数据弹窗 UI 实现 | ✅ 已完成 | 使用 shadcn/ui Dialog |
| 交互逻辑优化 | ✅ 已完成 | 增加 Loading 锁定 |
| 多语言适配 | ✅ 已完成 | 中英文词条添加 |
