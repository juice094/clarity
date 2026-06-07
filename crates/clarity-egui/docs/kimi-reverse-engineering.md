# Kimi Desktop v3.0.15 — 前端逆向分析文档

> Source: `%LOCALAPPDATA%/Temp/kimi-asar-extract/app/out/renderer/assets/`
> Extracted: 2026-06-07
> Scope: `ConversationView` CSS/JS chunks + `common` design tokens

---

## 1. 架构概览

Kimi Desktop v3 采用 **Electron + Vue 3 + Naive UI**，构建工具为 **Vite**（代码分割为按路由的 lazy chunks）。

ConversationView 相关的关键文件：

| File | Size | Content |
|------|------|---------|
| `ConversationView-C850IKYy.js` | ~71 KB | Vue SFC compiled JS (components: MsgBubble, ThinkBlock, ToolGroup, SubagentGroup, ApprovalPrompt, KnowledgePanel, MessageList, Composer) |
| `ConversationView-qy4YIzrk.css` | ~8 KB | Scoped CSS for all ConversationView sub-components |
| `common-Dd23LXub.css` | ~182 KB | Global design tokens (`:root` / `:root.dark`), shared component styles (Button, Checkbox, Modal, Tooltip, Image) |
| `index-CT278WWz.css` | ~2 KB | Theme transition animation (`kimi-theme-circle-reveal`) |

---

## 2. 设计令牌（Design Tokens）

### 2.1 语义色彩

| Token | Light Mode | Dark Mode | Usage |
|-------|-----------|-----------|-------|
| `--Colors-KMBlue` | `#1783ff` | `#1a88ff` | Primary action, links, accent |
| `--Colors-Red` | `#ff3849` | `#ff4756` | Error, danger, reject |
| `--Colors-PositiveGreen` | `#16c456` | `#16c456` | Success, done status |
| `--Colors-Orange` | `#ff9500` | `#ff9f0a` | Warning, approval badge |
| `--Colors-Yellow` | `#ffd230` | `#ffd230` | Highlight |
| `--Colors-Purple` | `#985ffb` | `#a16bff` | Special accent |

### 2.2 背景层级

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--Bg-Primary` | `#ffffff` | `#121212` | App main background |
| `--Bg-Secondary` | `#f5f5f5` | `#1f1f1f` | Code blocks, secondary surfaces |
| `--Bg-Tertiary` | `#ffffff` | `#292929` | Cards, approval prompt bg |
| `--Bg-GroundPC` | `#f9fbfc` | `#161717` | Ground/ canvas layer |
| `--Bg-Quaternary` | `#ffffff` | `#4d4d4d` | Elevated surfaces |
| `--BgGp-Secondary` | `#ffffff` | `#1f1f1f` | Group panels, composer bg |

### 2.3 文字层级

| Token | Light (alpha on black) | Dark (alpha on white) | Usage |
|-------|------------------------|----------------------|-------|
| `--Labels-Primary` | `rgba(0,0,0,.9)` | `rgba(255,255,255,.84)` | Headings, primary text |
| `--Labels-Secondary` | `rgba(0,0,0,.6)` | `rgba(255,255,255,.56)` | Body, secondary text |
| `--Labels-Tertiary` | `rgba(0,0,0,.45)` | `rgba(255,255,255,.42)` | Captions, placeholders |
| `--Labels-Quaternary` | `rgba(0,0,0,.3)` | `rgba(255,255,255,.26)` | Disabled, chevrons |

### 2.4 填充层级

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--Fills-F1` | `rgba(0,0,0,.03)` | `rgba(255,255,255,.05)` | Subtle hover bg |
| `--Fills-F2` | `rgba(0,0,0,.05)` | `rgba(255,255,255,.1)` | User bubble bg (light), hover |
| `--Fills-F3` | `rgba(0,0,0,.15)` | `rgba(255,255,255,.18)` | Disabled, scrollbar |
| `--Fills-F4` | `rgba(0,0,0,.25)` | `rgba(255,255,255,.25)` | Borders, checkbox |

### 2.5 分隔线

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--Separators-S1` | `rgba(0,0,0,.13)` | `rgba(255,255,255,.12)` | Card borders, dashed rails |

### 2.6 特殊语义

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--Others-BubbleBlue` | `#1783ff` | `#292929` | User bubble tint (not actually used in PC) |
| `--Others-BubbleGray_PC` | `#f5f5f5` | `#292929` | User bubble bg on PC |
| `--Others-LightOrangeBg` | `rgba(255,149,0,.1)` | `rgba(255,159,10,.1)` | Approval badge bg |
| `--Others-LightRedBg` | `rgba(255,77,77,.1)` | `rgba(255,82,82,.1)` | Error block bg |
| `--Others-TextSelected` | `rgba(23,131,255,.2)` | `rgba(26,136,255,.2)` | Selection highlight |

---

## 3. 排版系统

### 3.1 UI 字号

| Token | Size | Line-Height | Usage |
|-------|------|-------------|-------|
| `--ui-T1` | 18px | 26px | Large titles |
| `--ui-T2` | 16px | 24px | Section headers, buttons |
| `--ui-B1` | 15px | 22px | Body emphasis |
| `--ui-B2` | 14px | 20px | Body, captions, plan steps |
| `--ui-C1` | 12px | 18px | Small labels, badges |
| `--ui-C2` | 10px | 14px | Tiny |

### 3.2 Markdown 内容字号

| Token | Size | Line-Height | Usage |
|-------|------|-------------|-------|
| `--markdown-H1_Content` | 22px | 36px | H1 |
| `--markdown-H2_Content` | 20px | 32px | H2 |
| `--markdown-H3_Content` | 18px | 28px | H3 |
| `--markdown-B1_Content` | 16px | 26px | Body (assistant messages) |
| `--markdown-B2_Content` | 15px | 24px | Body secondary |
| `--markdown-B3_Content` | 14px | 22px | Compact body |
| `--markdown-Codeblocks` | 14px | 22px | Code blocks |
| `--markdown-InlineCode` | 16px | 26px | Inline code |

### 3.3 字体栈

```css
font-family: var(--Font-Family-Base);
/* Inferred from system: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif */
```

Special: `font-family: Pixelify, monospace` for subagent ordinals (tabular nums).

---

## 4. 组件布局规则

### 4.1 页面架构（ConversationView）

```
.conversation-view [flex row, height: 100%]
  ├── .conv-main [flex column, flex: 1]
  │     ├── .conv-header [flex row, min-height: 40px, padding: 0 16px 0 10px]
  │     │     ├── .conv-title-chip [title + chevron, -webkit-app-region: no-drag]
  │     │     └── .conv-header-actions [icon buttons, 32x32px]
  │     ├── .messages [flex: 1, overflow-y: auto]
  │     │     └── .message-list-inner [max-width: 800px, margin: 0 auto]
  │     ├── .approval-dock [flex-shrink: 0, padding: 0 16px]
  │     │     └── .approval-inner [max-width: 800px, margin: 0 auto 10px]
  │     └── .composer-dock [flex-shrink: 0, padding: 0 16px 16px]
  │           └── .composer-inner [max-width: 800px, margin: 0 auto]
  └── .knowledge-panel [width: 386px, flex-shrink: 0]  (optional right panel)
```

**Responsive breakpoints:**
- `@media(max-width: 840px)`: `.message-list-inner`, `.approval-inner`, `.composer-inner` → `max-width: 80%`
- `@media(max-width: 568px)`: → `max-width: 100%`

### 4.2 Message Bubble（msg）

**DOM Structure:**
```
.msg [display: flex, width: 100%]
  ├── .msg-avatar [56x56px, flex-shrink: 0, margin-right: -12px]  (assistant only)
  └── .bubble [display: flex, flex-direction: column, gap: 4px]
        ├── .blocks [flex column, gap: 12px, font: 16px/26px]
        │     └── .block [min-width: 0]  (text, think, tool, subagent, error...)
        └── .msg-footer [position: absolute, top: 100%, left: 16px]
              └── .msg-copy [opacity: 0 → 1 on :hover]
```

**User message (.msg-user):**
- `flex-direction: row-reverse`
- `.role-user`: `max-width: 80%`, `padding: 12px 16px`, `border-radius: 12px`
- Background: `var(--Fills-F2)` (light: `rgba(0,0,0,.05)` ≈ `#f5f5f5`)
- Text color: `var(--Labels-Primary)`
- `.blocks`: `font-size: 16px`, `line-height: 26px`
- `.block-text`: `word-break: break-word`, `white-space: pre-wrap`

**Assistant message (.msg-assistant):**
- `align-items: flex-start`
- `.role-assistant`: `flex: 1`, `min-width: 0`, `padding: 12px 0 12px 16px`
- Text color: `var(--Labels-Primary)`
- No background tint on assistant bubble

**Avatar:**
- Size: `56x56px`
- `display: flex`, `align-items: center`, `justify-content: center`
- `margin-right: -12px` (negative margin overlaps with bubble)
- `cursor: pointer`

**Copy button:**
- `.msg-copy`: `opacity: 0`, transitions to `opacity: 1` on `.msg:hover`
- Transition: `opacity .15s ease`

### 4.3 Message List（message-list）

```
.message-list [position: relative, flex: 1, min-height: 0, flex column]
  └── .message-scroller [flex: 1, flex column, overflow-y: auto]
        └── .message-list-inner [max-width: 800px, margin: 0 auto]
              [padding: 16px 16px 30px 4px]
              [display: flex, flex-direction: column, gap: 12px]
```

**Scroller styling:**
- `scrollbar-gutter: stable both-edges`
- WebKit scrollbar: `width: 6px`, `background: transparent`
- Thumb: `background: transparent` default → `var(--Fills-F2)` when `.is-scrolling`
- `border-radius: 3px`

**Scroll-to-bottom button (.to-bottom):**
- `position: absolute`, `bottom: 16px`, `right: 16px`
- `38x38px`, `border-radius: 50%`
- `border: 1px solid var(--Separators-S1)`
- `background: var(--BgGp-Secondary)`
- `box-shadow: 0 4px 10px #00000040`
- `opacity: 0` → `opacity: 1` (`.to-bottom-show`)
- Hover: `background: var(--BgGp-Secondary-hover)`

### 4.4 Thinking Block（think-block）

```
.think-block [min-width: 0]
  ├── .think-summary [flex row, align-items: center, gap: 8px]
  │     ├── .think-label [flex: 0 1 auto, ellipsis, 16px/24px, color: --Labels-Secondary]
  │     └── .think-chevron [16px width, --Labels-Quaternary, opacity: 0 → 1]
  └── .think-body [position: relative, margin-top: 8px, max-height: 340px, overflow-y: auto]
        └── .think-fade-mask [position: sticky, bottom: 0, height: 40px]
              [gradient: transparent → --Bg-Primary, opacity: 0 → 1]
```

**Behavior:**
- Chevron: `opacity: 0` default, `opacity: 1` on `.expanded` or `:hover`
- Label color: `--Labels-Secondary` → `--Labels-Primary` on hover (unless `.shimmer-text`)
- Chevron rotation: `transform: rotate(90deg)` when `.rotated`
- Body scroll: custom WebKit scrollbar (`width: 4px`, `--Fills-F2` thumb)
- Fade mask: sticky bottom gradient, `opacity: 0` → `opacity: 1` when `.visible`

**Shimmer text (loading state):**
- Gradient animation: `--Labels-Tertiary` → `--Labels-Primary` → `--Labels-Tertiary`
- `background-size: 200% 100%`
- `animation: think-shimmer 1.8s linear infinite`
- Reduced motion: fallback to static `--Labels-Secondary`

### 4.5 Tool Group（tool-group）

```
.tool-group [min-width: 0]
  ├── .tool-group-summary [flex row, align-items: center, gap: 8px, cursor: pointer]
  │     ├── .tool-label [ellipsis, 16px/24px, --Labels-Secondary]
  │     └── .tool-chevron [12px, --Labels-Tertiary, rotate 90deg when expanded]
  └── .tool-group-body [flex column]
        └── .tool-row [flex row, gap: 8px, align-items: flex-start]
              ├── .tool-rail [20px width, 32px height, flex center]
              │     └── .tool-glyph [color: --Labels-Secondary]
              └── .tool-content [flex: 1, min-width: 0]
                    └── .tool-row-head [flex row, align-items: center, gap: 8px]
                          ├── .tool-label
                          └── .tool-chevron [16px, --Labels-Quaternary]
                    └── .tool-detail [flex column, gap: 8px, padding: 4px 0 8px]
                          └── .tool-section [flex column, gap: 10px, padding: 12px 4px 4px 12px]
                                ├── .tool-section-label [14px/20px, 500 weight, --Labels-Primary]
                                └── .tool-code [max-height: 120px, overflow: auto]
```

**Timeline rail:**
- `.tool-row:not(:last-child)`: `padding-bottom: 16px`
- Dashed vertical line: `::before` pseudo-element
  - `position: absolute`, `left: 10px`, `top: 34px`, `bottom: 2px`
  - `border-left: 1px dashed var(--Separators-S1)`

**Interactive states:**
- `.tool-row-head.interactive`: `cursor: pointer`
- Chevron opacity: `0` default → `1` on hover or when `.rotated`
- Label color: `--Labels-Secondary` → `--Labels-Primary` on hover

### 4.6 Subagent Group（subagent-group）

```
.subagent-group [min-width: 0]
  ├── .subagent-summary [flex row, align-items: center, gap: 8px, cursor: pointer]
  │     ├── .subagent-icon [flex-shrink: 0, color: --Labels-Secondary]
  │     ├── .subagent-title [ellipsis, 16px/24px, --Labels-Secondary]
  │     └── .subagent-chevron [16px, --Labels-Quaternary, opacity: 0 → 1]
  └── .subagent-body [flex row, gap: 8px, margin-top: 4px]
        ├── .subagent-rail [20px width, align-self: stretch]
        │     └── ::before [dashed border-left: 1px dashed --Separators-S1]
        └── .subagent-card [flex: 1, min-width: 0, bg: --Fills-F1, radius: 12px]
              └── .subagent-scroll [max-height: 208px, overflow-y: auto, padding: 8px]
                    └── .subagent-row [flex row, align-items: center, gap: 8px, padding: 10px 12px]
                          ├── .subagent-ordinal [Pixelify mono, 14px/20px, tabular-nums, --Labels-Secondary]
                          ├── .subagent-desc [ellipsis, 14px/20px, --Labels-Secondary]
                          └── .subagent-status [14px/20px, white-space: nowrap]
```

**Status colors:**
- `.is-running`: `--Labels-Tertiary`
- `.is-failed`: `--Colors-Red`
- `.is-succeeded`: `--Colors-PositiveGreen`

### 4.7 Approval Prompt（approval-prompt）

```
.approval-prompt [flex column, gap: 8px, padding: 16px]
  ├── .ap-header [flex row, align-items: flex-start, justify-content: space-between, gap: 12px]
  │     ├── .ap-title-wrap [flex row, align-items: center, gap: 11px, min-width: 0]
  │     │     ├── .ap-dot [6x6px, border-radius: 50%, bg: --Colors-Orange]
  │     │     ├── .ap-title [ellipsis, 16px/24px, 500 weight, --Labels-Primary]
  │     │     └── .ap-badge [padding: 1px 6px, radius: 21px, bg: --Others-LightOrangeBg, color: --Colors-Orange, 12px/18px]
  │     └── [close/dismiss action]
  ├── .ap-content [flex column, gap: 4px]
  │     └── .ap-detail [overflow-y: auto, max-height: 80px, 14px/20px, --Labels-Tertiary, word-break: break-all]
  └── .ap-actions [flex row, align-items: center, justify-content: flex-end, gap: 8px]
        ├── .ap-btn.ap-deny [bg: --Fills-F1, color: --Labels-Primary, min-width: 62px, padding: 6px 8px 6px 10px, radius: 10px]
        │     └── .ap-key [24x18px, radius: 4px, bg: --Fills-F1, color: --Labels-Quaternary]
        └── .ap-btn.ap-allow [bg: --Labels-Primary, color: --Bg-Primary, same sizing]
              └── .ap-key [bg: #ffffff0d, color: --Bg-Primary]
```

**Card styling:**
- `border: .5px solid var(--Separators-S1)`
- `border-radius: 20px`
- `background: var(--Bg-Tertiary)`
- `box-shadow: 0 5px 8px #00000012`

**Button hover:**
- `.ap-deny:hover`: `background: var(--Fills-F2)`
- `.ap-allow:hover`: `opacity: .9`

### 4.8 Knowledge Panel（knowledge-panel）

```
.knowledge-panel [width: 386px, height: 100%, overflow-y: auto, flex column, align-items: center, gap: 16px, padding: 16px]
  ├── .knowledge-card [width: 354px, flex column, bg: --Bg-Primary, border: .5px solid --Separators-S1, radius: 16px, padding: 16px]
  │     ├── .card-header [flex row, align-items: center, justify-content: space-between, gap: 8px, cursor: pointer]
  │     │     ├── .card-title [14px/20px, 500 weight, --Labels-Primary]
  │     │     └── .card-header-trailing [flex row, align-items: center, gap: 8px]
  │     ├── .plan-summary [14px/20px, --Labels-Tertiary]
  │     └── .plan-list [flex column, gap: 12px, max-height: 340px, overflow-y: auto]
  │           └── .plan-step [flex row, align-items: center, gap: 8px]
  │                 ├── .plan-check [20x20px, flex center]
  │                 │     └── .plan-spinner [animation: plan-spin .9s linear infinite]
  │                 └── .plan-step-title [14px/20px, ellipsis]
  │                       [.done → text-decoration: line-through]
  │                       [.pending → color: --Labels-Tertiary]
  │                       [.shimmer-text → animated gradient]
  └── .knowledge-card--context [same card, flex: 0 1 auto, min-height: 0]
        ├── [same header]
        ├── .context-list [flex column, gap: 0, flex: 1 1 auto, min-height: 0, overflow-y: auto]
        │     └── .context-item [flex row, align-items: center, justify-content: space-between, gap: 8px, padding: 8px, radius: 8px, cursor: pointer]
        │           ├── .context-item-main [flex row, align-items: center, gap: 8px, min-width: 0]
        │           │     ├── .context-doc-icon [flex-shrink: 0, color: --Labels-Secondary]
        │           │     └── .context-file-name [14px/20px, ellipsis, --Labels-Secondary]
        │           └── .context-reveal [flex-shrink: 0, opacity: 0 → 1 on hover/focus]
        └── .context-toggle [flex row, align-items: center, gap: 8px, padding: 8px 8px 0 0, cursor: pointer]
              └── .context-toggle-chevron [transition: transform .15s ease, rotate 180deg when expanded]
```

**Empty states:**
- `.progress-empty`: `153x62px` illustration
- `.context-empty`: `131x50px` illustration with 3 overlapping cards (rotated)

### 4.9 Error Block（block-error）

```
.block-error [flex row, align-items: flex-start, gap: 8px, padding: 10px 12px, radius: 10px, bg: --Others-LightRedBg]
  ├── .block-error-icon [flex-shrink: 0, margin-top: 2px, color: --Colors-Red]
  └── .block-error-text [color: --Labels-Primary, word-break: break-word]
```

### 4.10 Chat Markdown（chat-markdown）

```
.chat-markdown [font-size: 14px, line-height: 1.6, color: --Labels-Primary, min-width: 0, word-break: break-word]
  ├── pre [overflow-x: auto, bg: --Bg-Secondary, radius: 6px, padding: 12px, margin: 8px 0]
  ├── code [font-size: 13px]
  ├── p [margin: 0 0 8px]
  └── .file-mention-chip [inline-flex, padding: 0 5px, radius: 4px, bg: --Fills-F1, color: --Labels-Secondary, 13px/20px]
```

**Streaming cursor:**
- `.is-streaming .markdown > *:last-child::after`
- `content: ""`, `display: inline-block`, `width: 14px`, `height: 14px`
- `background-image: var(--moon-loading)` (light/dark apng)
- `background-size: contain`, `position: relative`, `top: 2px`

### 4.11 Cron Card（参考：共享组件）

```
.cron-card [flex column, gap: 16px, padding: 16px, border: .5px solid --Separators-S1, radius: 16px, cursor: pointer]
  ├── .card-head [flex row, align-items: flex-start, gap: 10px]
  │     ├── .card-head-main [flex column, gap: 4px, min-width: 0]
  │     │     ├── .card-title-row [flex row, align-items: center, gap: 6px]
  │     │     │     ├── .job-name [ellipsis, 16px/24px, 500 weight, --Labels-Primary]
  │     │     │     └── .job-schedule [ellipsis, 14px/20px, --Labels-Secondary]
  │     │     └── .job-description [-webkit-line-clamp: 2, 14px/20px, --Labels-Tertiary]
  │     └── .card-actions [flex row, align-items: center, gap: 12px]
  │           └── .more-btn [18x18px, radius: 6px, transparent, --Labels-Secondary]
  └── .switch [32x18px, radius: 999px, bg: --Fills-F3, cursor: pointer]
        └── .switch-dot [14x14px, radius: 50%, bg: --Always-White, translateX(14px) when .on]
```

---

## 5. 动画与过渡

| Animation | Duration | Easing | Usage |
|-----------|----------|--------|-------|
| `think-shimmer` | 1.8s | linear infinite | Loading text gradient |
| `tool-shimmer` | 1.8s | linear infinite | Tool label loading |
| `subagent-shimmer` | 1.8s | linear infinite | Subagent title loading |
| `plan-shimmer` | 1.8s | linear infinite | Plan step loading |
| `compaction-shimmer` | 1.8s | linear infinite | Compaction row |
| `plan-spin` | 0.9s | linear infinite | Plan step spinner |
| Chevron rotate | 0.15s | ease | Expand/collapse |
| Opacity (hover) | 0.15s | ease | Copy button, context reveal |
| Opacity (scroll btn) | 0.3s | ease-in-out | Scroll-to-bottom |
| Background transition | 0.15s | ease | Card hover, button hover |
| Theme reveal | 0.4s | cubic-bezier(.16,1,.3,1) | Dark/light switch |

**Reduced motion (`prefers-reduced-motion: reduce`):**
- All shimmer animations → `animation: none`
- Fallback to static color (`--Labels-Secondary`)

---

## 6. 滚动条规范

| Context | Width | Track | Thumb | Hover Thumb |
|---------|-------|-------|-------|-------------|
| Message scroller | 6px | transparent | transparent / `--Fills-F2` (when scrolling) | - |
| Think body | 4px | transparent | `--Fills-F2` | `--Fills-F3` |
| Tool code | 8px | transparent | transparent / `--Fills-F2` (when `.is-scrolling`) | - |
| Subagent scroll | 8px | transparent | `--Fills-F2` (with 2px border padding) | `--Fills-F3` |
| Knowledge plan/context | 6px | - | `--Fills-F2` | `--Fills-F3` |
| Approval detail | 4px | transparent | `--Fills-F2` | `--Fills-F3` |

---

## 7. 到 Clarity Theme 的映射建议

| Kimi Token | Clarity `Theme` Field | Notes |
|-----------|----------------------|-------|
| `--Bg-Primary` | `bg` | Main canvas |
| `--Bg-Secondary` | `surface` / `code_block_bg` | Secondary surfaces |
| `--Labels-Primary` | `text` | Primary text |
| `--Labels-Secondary` | `text_dim` | Secondary text |
| `--Labels-Tertiary` | `text_dim` (lighter) | Captions |
| `--Fills-F2` | `surface` | User bubble (light mode) |
| `--Separators-S1` | `border` | Borders, separators |
| `--Colors-KMBlue` | `accent` | Primary accent |
| `--Colors-Red` | `danger` | Error |
| `--Colors-PositiveGreen` | `ok` | Success |
| `--Colors-Orange` | `status_busy` | Warning |
| `--Others-LightOrangeBg` | `status_busy` (with alpha) | Badge bg |
| `--Others-LightRedBg` | `danger` (with alpha) | Error block |
| `--Bg-Tertiary` | `surface` | Approval card bg |

**Clarity 当前缺失的 Kimi 语义：**
- `Fills-F1` (`rgba(0,0,0,.03)`) → 需要新增 `fill_subtle`
- `Labels-Quaternary` (`rgba(0,0,0,.3)`) → 需要新增 `text_very_dim`
- `BgGp-Secondary` → 需要新增 `panel_bg`
- `Always-White` → 需要新增 `always_white` (for dark mode buttons)

---

## 8. 组件实现状态（Clarity）

| Component | File | Status |
|-----------|------|--------|
| `message_bubble` | `conversation.rs` | Integrated (basic layout) |
| `thinking_block` | `conversation.rs` | Available (unwired) |
| `tool_group` | `conversation.rs` | Available (unwired) |
| `subagent_group` | `conversation.rs` | Available (unwired) |
| `approval_dock` | `conversation.rs` + `panels/chat/mod.rs` | Integrated |
| `knowledge_panel` | `conversation.rs` + `panels/workspace.rs` | Integrated |
| `step_list` | `conversation.rs` | Integrated (in knowledge_panel) |
| `collapsible` | `conversation.rs` | Available (shared primitive) |
| `streaming_cursor` | `conversation.rs` | Available (unwired) |

---

## 9. 文件位置备忘

```
%LOCALAPPDATA%/Temp/kimi-asar-extract/
└── app/
    ├── package.json           (v3.0.15, Electron + Vue 3 + Vite)
    ├── out/
    │   └── renderer/
    │       ├── assets/
    │       │   ├── ConversationView-C850IKYy.js      (71 KB, component logic)
    │       │   ├── ConversationView-qy4YIzrk.css     (8 KB, scoped styles)
    │       │   ├── common-Dd23LXub.css               (182 KB, design tokens + shared components)
    │       │   ├── index-CT278WWz.css                (2 KB, theme transition)
    │       │   └── [other view chunks...]
    │       └── index.html
    └── node_modules/          (Naive UI, Vue 3, etc.)
```

---

*Generated from ASAR extraction. For re-extraction: use `@electron/asar` package to extract `resources/app.asar` from Kimi Desktop installation.*
