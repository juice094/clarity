# Clarity UI Annotator Schema

> Version 1.0  
> Tool: `assets/ui_annotator.html`

## 文件格式

导出文件为 JSON，结构如下：

```json
{
  "image": "屏幕截图 2026-06-14 142955.png",
  "image_width": 1650,
  "image_height": 1237,
  "annotations": [
    {
      "id": "ann_abc123",
      "label": "input_bar",
      "role": "chrome",
      "priority": "fixed",
      "color": "red",
      "note": "底部输入栏，固定高度 110px",
      "x": 300,
      "y": 1107,
      "w": 850,
      "h": 110
    }
  ]
}
```

## 字段说明

### 顶层

| 字段 | 类型 | 说明 |
|------|------|------|
| `image` | string | 原始图片文件名 |
| `image_width` | number | 原始图片宽度（像素） |
| `image_height` | number | 原始图片高度（像素） |
| `annotations` | array | 标注框列表 |

### 每个标注框

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string | 唯一标识 |
| `label` | string | 语义名称，如 `right_rail_toggle`、`session_list` |
| `role` | string | 见下文角色定义 |
| `priority` | string | 见下文优先级定义 |
| `color` | string | `red` / `green` / `blue` / `yellow` |
| `note` | string | 人类给 AI 的说明 |
| `x`, `y`, `w`, `h` | number | 以原始图片为基准的像素坐标 |

## 角色（role）

| role | 含义 | 默认颜色 |
|------|------|----------|
| `chrome` | 标题栏、边栏、输入栏等不随内容滚动的 UI | red |
| `content` | 主内容区、聊天流、可滚动区域 | green |
| `rail` | 左侧或右侧抽屉面板 | blue |
| `floating` | 浮层、弹出菜单、警告提示 | yellow |

## 优先级（priority）

| priority | 含义 |
|----------|------|
| `fixed` | 固定尺寸，响应式变化时保持不变 |
| `stretch` | 撑满父容器 |
| `remainder` | 占用剩余空间 |

## 标注规范与常见陷阱

1. **颜色语义强制统一**：`chrome=red`、`content=green`、`rail=blue`、`floating=yellow`。禁止将 rail 标为 green 或将 chrome 标为 blue，否则与 `clarity-egui` 红绿蓝黄诊断覆盖层对不上。
2. **避免父子区域重叠**：例如 `chat_content`（content）与 `input_bar`（chrome）不应上下重叠；应让 `chat_content` 位于 header 与 input_bar 之间，`input_bar` 作为同级 chrome 固定在底部。
3. **绝对值只作快照**：`x/y/w/h` 是相对于当前图片的像素值。AI 转译为 egui 代码时应换算为比例或布局约束（如 `SidePanel::left().width(200.0)`、`CentralPanel` + `max_width` 居中），不要硬编码当前图片的像素。
4. **推荐比例参考**（基于 1280×800 默认窗口与概念图）：
   - 左 rail（含 icon rail + 展开列表）总宽约 236px（≈18% 窗口宽）。
   - 右 rail 默认宽度 240px，可按内容在 180~360px 之间调整。
   - 中间内容区宽度随窗口变化；内部聊天列建议 `max_width` 约 720~800px，在宽屏下自动居中并留白。
   - header / input_bar 固定高度，不要随窗口高度按比例缩放。

## 使用流程

1. 打开 `assets/ui_annotator.html`。
2. 加载目标图片（点击“加载图片”或拖拽到画布）。
3. 切换到“画框”模式，在图片上拖拽绘制矩形。
4. 在右侧属性面板填写 `label`、`role`、`priority`、`note`。
5. 点击“导出 JSON”保存标注文件。
6. 将 JSON 文件发送给 AI，AI 读取坐标和语义信息。

## AI 反向协作

AI 可以生成 JSON 标注文件并交给用户加载：

1. AI 把代码中已实现的布局 rect 导出为上述 JSON。
2. 用户在 `ui_annotator.html` 中加载 JSON。
3. 用户看到 AI 画的候选框，拖动修正后再次导出。
4. AI 读取修正后的 JSON 更新实现。

## 快捷键

| 操作 | 快捷键 |
|------|--------|
| 切换选择模式 | 点击“选择” |
| 切换画框模式 | 点击“画框” |
| 删除选中框 | Delete / Backspace |
| 缩放画布 | 滚轮 |
| 平移画布 | 按住空格 + 拖拽 |
| 移动框体 | 选择模式下拖拽框体 |
| 缩放框体 | 选择模式下拖拽四角/四边控制点 |
