# Codex 页面入口与选择器

Codex class 名可能随版本变化。这里记录的是稳定入口和定位方法，不是允许盲目复制的永久 API。
每次 Codex 更新后都要用 CDP 复核。

## 全局窗口骨架

### Windows 顶部应用菜单栏

- 用户入口：窗口最上方“文件、编辑、视图、帮助”。
- 特征：`[class~="app-header-tint"][class*="application-menu-top-bar"]`
- 透明度：内容区 `--cbg-surface-opacity`。
- 注意：它位于 `main.main-surface` 外，必须单独打底。

### 左侧导航栏

- 用户入口：Codex 标题、新建任务、拉取请求、站点、已安排、插件、项目和任务列表。
- 稳定入口：`aside.app-shell-left-panel`
- 透明度：`--cbg-sidebar-opacity`。
- 内部 `nav` 应透明，避免与 aside 叠加。

### 主内容根

- 稳定入口：`main.main-surface`
- 规则：自身透明，不在这里打内容区底色。
- 内容区唯一打底层：`.app-shell-main-content-viewport`
- 透明度：`--cbg-surface-opacity`。
- `.app-shell-main-content-frame`、`[role="main"]` 和全高页面壳应透明。

### 首页与任务页识别

- 首页检测：`[role="main"]:has([data-testid="home-icon"])`
- 根 class：`codex-background-home` 或 `codex-background-task`
- 背景强度：`homeIntensity`、`taskIntensity`
- 不要按 URL 猜路由；Codex 内部导航可能不改变可依赖的普通 URL。

## 左侧入口页面

### 新建任务 / 首页

- 用户入口：左侧“新建任务”。
- 主内容：普通 `[role="main"]`，显示“我们该构建什么？”和 composer。
- 首页横幅：`.home-banners > aside[class*="bg-token-main-surface-primary"]`
- 横幅示例：“启用快速模式”。
- 横幅透明度：`--cbg-menu-opacity`；清原生 shadow，不隐藏文字和按钮。

### 拉取请求

- 用户入口：左侧“拉取请求”。
- 主内容使用内容区透明度。
- 打开详情后可能出现右侧辅助栏；右侧栏必须单独由菜单/面板透明度打底。
- 检查左右区域是否因主内容底色和右栏底色重叠而深浅不一致。

### 站点

- 用户入口：左侧“站点”。
- 可能出现全页壳：
  `[class~="h-full"][class~="min-h-0"][class~="flex-col"][class*="bg-token-main-surface-primary"]`
- 搜索输入 id：`#appgen-site-search`
- 不要写死 id 处理外层；站点、已安排、插件共享相同 sticky 搜索结构。

### 已安排

- 用户入口：左侧“已安排”。
- 搜索输入 id：`#scheduled-page-search`
- 搜索外层是带 `bg-token-main-surface-primary` 和 `::after` 渐变的 sticky。
- 清 sticky 实底和 `::after`，输入框自身继续跟随 composer 透明度。

### 插件

- 用户入口：左侧“插件”。
- 搜索输入 id：`#plugins-page-search`
- 与站点、已安排共享 sticky 搜索规则。
- 页面长列表不应给每个列表分组重复打内容区底色。

### 通用页面搜索栏

稳定结构：

```css
.app-shell-main-content-viewport
  [class~="sticky"][class*="bg-token-main-surface-primary"]:has(input[type="text"])
```

处理内容：

- sticky 的 `background-color` 透明；
- sticky 的 `::after` 背景和渐变关闭；
- 内部 `div.no-drag:has(> input[type="text"])` 跟随 `--cbg-composer-opacity`。

## 输入和弹层

### Composer

- 稳定入口：`.composer-surface-chrome`
- 兼容入口：`div.no-drag:has(> textarea)`
- 透明度：`--cbg-composer-opacity`
- 必须清：
  - `box-shadow`
  - `border-color`
  - `backdrop-filter`
  - `bg-gradient-to-t`、`from-token-main-surface-primary` 等底部渐变

### 普通搜索输入

- 兼容入口：`div.no-drag:has(> input[type="text"])`
- 透明度：`--cbg-composer-opacity`
- 不要让外部 sticky 再叠一层相同色。

### 下拉菜单、右键菜单、命令面板

- `[role="menu"]`
- `[role="listbox"]`
- `[class*="bg-token-dropdown-background"]:not(.composer-surface-chrome)`
- 透明度：`--cbg-menu-opacity`
- 内部同 token 子层透明，避免菜单内部再叠色。

## 右侧辅助栏

### 稳定外层

```css
main.main-surface aside[class~="ml-auto"][class*="z-[41]"]
```

- 透明度：`--cbg-menu-opacity`
- 它承载审阅、文件树、浏览器、终端等不同内容。
- 只给这个稳定 aside 打一层底；内部 `bg-token-main-surface-primary` 默认透明。

### 审阅

- 用户入口：任务页右上方“审阅”。
- 文件卡片：`.codex-review-diff-card`
- diff 自定义元素：`diffs-container`
- 文件标题 sticky：`.codex-review-diff-card > [class~="sticky"][class~="backdrop-blur-sm"]`
- 普通 CSS 只能处理宿主，不能进入 diff 的 Shadow DOM。

Shadow DOM 内部关键结构：

- `[data-diffs-header]`
- `[data-diff]`、`[data-file]`
- `[data-line-type="change-addition"][data-line]`
- `[data-line-type="change-deletion"][data-line]`

实现要求：

- 普通文档 CSS 先在 `diffs-container` 宿主固定 `--diffs-bg`、surface、context、
  separator 和 hover 变量；真实 diff 内容出现前的占位阶段也必须透明；
- 文档早期接管 `Element.prototype.attachShadow`；
- `diffs-container` 创建 shadow 后，在首帧前追加 `REVIEW_SHADOW_CSS`；
- Shadow CSS 必须覆盖 `:host`，不能只覆盖加载完成后才出现的 `[data-diff]`；
- 样式节点必须位于 Shadow DOM 原生样式之后；
- 新增/删除行保留低强度绿/红，不能直接抹掉审阅语义；
- cleanup 恢复原始 `attachShadow` 并移除 Shadow style；
- MutationObserver/4 秒 timer 只能作为兜底，不能制造可见延迟。

### 文件树

- 用户入口：审阅右侧“文件”区域或文件按钮。
- 自定义元素：`file-tree-container`
- 宿主背景透明，并设置：
  `--color-token-main-surface-primary: transparent`
- CSS 自定义属性会继承进 Shadow DOM，因此文件树不需要单独注入 Shadow style。

### 浏览器

- 仍使用同一个右侧 aside。
- 先检查内部是否新增不透明 `bg-token-main-surface-primary` 壳。
- 不要按内部按钮文本识别整个右栏；按钮和内容会动态切换。

### 集成终端

- 面板 id：`[id^="terminal-panel-"]`
- 外部壳：`div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"])`
- 工具栏：直接子级 `[class~="h-toolbar-pane"]`
- 透明度：`--cbg-terminal-opacity`

处理顺序：

1. 外部多层壳透明。
2. 仅终端面板和必要工具栏打一层 terminal 底色。
3. 工具栏内部子层透明。
4. 清 `.xterm-rows span[class*="xterm-bg-"]` 的 ANSI 字符背景。
5. 清 `.xterm-selection` 和 `.xterm-selection-layer` 的实色选区。
6. 对 `.xterm-bg-257.xterm-fg-257` 恢复可读前景色。

## 透明度归属

- 背景媒体：`--cbg-opacity`
- 首页强度：`--cbg-home-intensity`
- 任务页强度：`--cbg-task-intensity`
- 左侧栏：`--cbg-sidebar-opacity`
- 主内容区、顶部菜单栏：`--cbg-surface-opacity`
- composer、搜索输入：`--cbg-composer-opacity`
- 菜单、右侧辅助栏、首页横幅：`--cbg-menu-opacity`
- 集成终端：`--cbg-terminal-opacity`

所有值范围均为 0 到 1。

## 新页面定位步骤

1. 进入目标页面并截图。
2. 从异常区域中心用 `document.elementsFromPoint()` 获取元素栈。
3. 找到第一个非透明背景、渐变、shadow 或 backdrop-filter。
4. 沿祖先链确认它是页面壳、卡片还是稳定面板。
5. 检查 `::before`、`::after`。
6. 如果是自定义元素，检查 `shadowRoot` 和内部 CSS 变量。
7. 先在 CDP 临时设置样式并截图对比。
8. 再把精确规则写入 payload，补测试并删除探查文件。

## 视觉回归重点

- 相同不透明度的左右区域看起来应同深度。
- 页面导航后不能恢复原生实底。
- 滚动到新 diff 卡片时不能先黑后透明。
- 透明度为 0 时仍保留文字、按钮、边框和 diff 语义。
- 浅色主题不能继续使用深色 surface，深色主题不能回落到浅灰色。
