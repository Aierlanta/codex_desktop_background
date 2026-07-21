---
name: codex-background-development
description: Maintains Codex Background Studio, including CDP injection, Codex page and panel transparency, Shadow DOM review styling, media/slideshow behavior, debugging, packaging, and release verification. Use when changing this repository or investigating a Codex desktop UI surface that does not follow background settings.
---

# Codex Background Studio 开发

## 目标

维护 Windows 官方 Codex 桌面应用的可逆背景工具。通过本机 CDP 注入样式和媒体，
不修改 `WindowsApps`、`app.asar`、应用签名、登录状态或对话数据。

动手前按需读取：

- 页面入口、DOM 特征、透明度归属：[windows-and-selectors.md](windows-and-selectors.md)
- 架构、故障经验、安全边界：[architecture-and-pitfalls.md](architecture-and-pitfalls.md)

## 不可破坏的原则

1. 不直接修改 Codex 安装资源；暂停和恢复必须能完整移除注入。
2. 不凭截图猜选择器。通过 Codex CDP 检查实际 DOM、计算样式和 Shadow DOM。
3. 一个视觉区域只保留一层有色背景。内部壳层透明，避免半透明叠加变暗。
4. 所有界面不透明度必须允许 `0`；不要添加最低 0.2 之类的兜底。
5. 不用宽泛选择器清空所有背景。先确认稳定入口、尺寸、层级和页面复用范围。
6. `backdrop-filter` 默认关闭；它会让相同数值的区域看起来深浅不同。
7. 动态组件必须首帧生效。不要依赖 200ms 之后的 MutationObserver 补丁来消除闪烁。
8. 一次性 CDP 探查脚本放在 `poc/`，验证完立即删除。
9. 修改 `BACKGROUND_CSS` 或 Shadow CSS 时必须进入修订哈希，确保现有会话热更新。
10. 更改共享设置时同步修改 contracts、默认值、规范化、UI、payload 和测试。

## 代码入口

- `src/main/index.ts`：Electron 主进程、IPC、媒体 payload、轮播、托盘和应用生命周期。
- `src/main/codex-controller.ts`：发现官方 Store 包、校验进程、启动 Codex、保存 CDP 会话。
- `src/main/injector.ts`：CDP target 同步、早期脚本、运行时更新、暂停和移除。
- `src/main/payload.ts`：Codex 页面背景层、CSS 变量、页面选择器、Shadow DOM 和清理逻辑。
- `src/main/settings.ts`：设置规范化、边界和事务写入。
- `src/shared/contracts.ts`：主进程、preload、renderer 共用类型、默认值和 IPC 名称。
- `src/main/media-library.ts`：媒体校验、目录、哈希、动态 API 刷新和文件替换。
- `src/main/network-policy.ts`：远程下载、DNS 固定、SSRF 防护、格式和大小限制。
- `src/main/preload.cts`、`src/renderer/bridge.ts`：安全 IPC 桥。
- `src/renderer/App.tsx`：Studio 操作界面和设置控件。
- `src/main/*.test.ts`：设置、媒体、网络、payload 回归测试。

## 标准开发流程

### 1. 建立基线

先看 Git 状态和当前版本，不覆盖用户已有改动。确认 Codex 和 Studio 是否已有运行实例。

```powershell
git status --short --branch
npm run check
```

开发要求 Node.js 22+。项目使用 npm，不要手工编造依赖版本。

### 2. 复现并确定视觉层

先判断问题属于哪一类：

- 页面主壳：通常归 `surfaceOpacity`。
- 左侧栏：归 `sidebarOpacity`。
- 输入框和搜索输入：归 `composerOpacity`。
- 菜单、右侧辅助栏、首页横幅：归 `menuOpacity`。
- 集成终端：归 `terminalOpacity`。
- 背景媒体本身：归 `opacity`、路由强度和遮罩设置。

不要用子层再画一层相同透明色。应让外层统一打底、内部壳透明。

### 3. 用 CDP 检查 Codex

运行时端口记录在 `%LOCALAPPDATA%/CodexBackgroundStudio/runtime.json`。只连接：

- `127.0.0.1` 回环地址；
- browser ID 与状态文件一致的实例；
- URL 以 `app://` 开头的 page target。

探查内容至少包括：

- 元素标签、id、role、完整 class；
- `getBoundingClientRect()`；
- `backgroundColor`、`backgroundImage`、`boxShadow`、`backdropFilter`；
- `::before`、`::after`；
- 元素祖先链；
- 自定义元素的 `shadowRoot` 和内部 CSS 变量。

截图验证前后状态。不要把探查脚本或截图提交进仓库。

### 4. 选择实现方式

- 普通 DOM：在 `BACKGROUND_CSS` 增加精确规则。
- Portal 菜单：使用 role 或稳定 dropdown token，注意排除 composer。
- 全页壳：清内部实底，只由 `.app-shell-main-content-viewport` 打底。
- 右侧辅助栏：由稳定的 `aside.ml-auto.z-[41]` 打底，内容页内部透明。
- Shadow DOM：外层 CSS 无法穿透。使用早期 `attachShadow` 接管，在首帧前注入，
  并保留定时扫描作为异常兜底。
- xterm：清 ANSI 背景类和选区层，不要降低整块终端文本透明度。

### 5. 保证首帧和可恢复性

`earlyPayloadFor()` 必须在 `documentElement` 出现时即可运行。全局 CSS 不应等待
Codex 主壳完成挂载。

动态更新使用 `requestAnimationFrame` 合并到下一帧绘制前，不使用肉眼可见的固定延迟。

新增任何注入对象时，同时补齐 `cleanup()`：

- 移除 style、layer 和 Shadow style；
- 恢复被包装的原型方法；
- 断开 observer、timer；
- 撤销 Blob URL；
- 删除根 class 和 CSS 变量。

### 6. 验证

```powershell
npm run check
```

随后使用真实 Codex 验证：

1. 首页和首页推荐横幅。
2. 新建任务及输入栏。
3. 拉取请求页和右侧面板。
4. 站点、已安排、插件页及 sticky 搜索栏。
5. 任务页审阅、文件树、浏览器和终端。
6. 深色、浅色主题。
7. 所有界面透明度为 `0`、中间值和 `1`。
8. 导航、滚动、新开面板和重载时无黑底闪烁。
9. 暂停、恢复官方外观后不残留样式或原型包装。

### 7. 版本和打包

补丁修复递增 patch 版本；功能或设置结构变化再考虑 minor 版本。同步修改：

- `package.json`
- `package-lock.json` 根版本和 packages 根版本

```powershell
npm run check
npm run package:win
```

安装包位于 `release/Codex Background Studio Setup <version>.exe`。

若 electron-builder 报 `EBUSY`，先关闭 `release/win-unpacked` 中的 Studio 进程，
等待 Windows/杀毒软件释放文件锁，再重试；不要删除用户数据。

### 8. 提交和传输

提交前检查完整 diff、测试结果、版本和临时文件。只在用户明确要求时提交或推送。
提交信息说明为什么改，而不是只罗列文件。

## 完成条件

- 目标页面视觉结果符合对应透明度控制；
- 没有透明度叠加、原生渐变、阴影或首帧闪烁；
- 真实 Codex 验证完成；
- `npm run check` 全部通过；
- 临时探查文件已删除；
- 安装包版本正确且可读取；
- 恢复流程仍完整可逆。
