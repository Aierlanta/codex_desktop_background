# 架构与故障经验

## CDP 注入链路

完整流程：

1. `src-tauri/src/controller.rs` 用 PowerShell 查找 `OpenAI.Codex` Store 包。
2. 校验包签名类型、Manifest、`app/ChatGPT.exe` 和 AppUserModelId。
3. 若已有未启用 CDP 的 Codex，要求用户确认重启。
4. 通过 `IApplicationActivationManager` 启动 Codex，参数只绑定 `127.0.0.1`。
5. 选择 9335 起的可用端口，并等待 `/json/version`。
6. 保存 port、browser ID、包名和 executable 到 `runtime.json`。
7. `src-tauri/src/injector.rs` 只接受同一 browser ID、回环 WebSocket、`app://` page target。
8. 为每个 target 开启 Runtime/Page、临时 bypass CSP。
9. `Page.addScriptToEvaluateOnNewDocument` 注册早期 payload。
10. `Runtime.evaluate` 立即应用当前页面。
11. 每 1200ms 同步新 target；导航和重载由早期脚本覆盖。

安全边界不要放松：

- 不连接任意调试端口。
- 不接受非回环 WebSocket。
- 不向非 `app://` target 注入。
- 不按进程名粗暴结束所有 `ChatGPT.exe`；必须比较官方包 executable 的完整路径。

## Payload 生命周期

`payload.ts` 生成自包含 IIFE，状态保存在：

```text
window.__CODEX_BACKGROUND_STUDIO__
```

主要对象：

- `#codex-background-style`：普通 DOM 样式。
- `#codex-background-layer`：固定背景媒体层。
- `#codex-background-media`：图片或视频。
- `#codex-background-tile`：平铺模式。
- `#codex-background-overlay`：颜色遮罩。
- `#codex-background-review-shadow-style`：每个 diff Shadow Root 内的审阅覆盖。

重应用前先调用旧状态的 `cleanup()`，避免：

- observer/timer 重复；
- Blob URL 泄漏；
- `attachShadow` 多层包装；
- style 和 layer 重复；
- 旧版本 revision 阻止新 CSS 生效。

修订号同时混入：

- 媒体 sha256；
- display 设置；
- 媒体 kind；
- `BACKGROUND_CSS`；
- `REVIEW_SHADOW_CSS`。

## 为什么媒体使用 base64 + Blob URL

Codex 的 `app://` 渲染页会阻止访问 Studio 的回环 HTTP 媒体 URL。即使 bypass CSP，
回环 fetch 仍可能被 scheme/sandbox 策略拦截。

因此 Rust 后端的 `active_payload()`：

1. 读取受管媒体文件；
2. 限制内嵌大小为 64 MB；
3. 生成 data URL；
4. payload 内解码为 Blob；
5. 使用 Blob URL 给 `<img>` 或 `<video>`；
6. cleanup 时 revoke。

媒体加载失败时必须整体 cleanup，不能留下黑色空背景层。

`preview.rs` 主要服务 Studio 预览和视频 Range，不应重新拿来作为 Codex 背景源。

## 媒体库与动态随机 API

数据目录：

```text
%LOCALAPPDATA%/CodexBackgroundStudio
```

主要文件：

- `settings.json`
- `library.json`
- `runtime.json`
- `media/`
- `temporary/`

动态 API 条目：

- `origin` 为 `api`；
- `sourceUrl` 保存用户输入的随机 API 地址，不保存最终重定向地址；
- 每次轮播或手动刷新重新请求；
- 即使播放列表只有一个动态条目也允许轮播；
- 下载失败保留当前缓存，不中断轮播；
- 预览 URL附加 sha256 查询参数避免缓存旧图。

Windows 文件锁注意：

- 不覆盖当前媒体文件；
- 新内容使用 `<id>-<hash-prefix>.<ext>`；
- catalog 更新后再最佳努力删除旧文件；
- 这是为了避免预览服务正在读取旧文件时产生 `EBUSY`。

## 网络安全

远程媒体只允许无账号信息的 HTTP/HTTPS。

每次请求和重定向都要：

1. 验证 URL；
2. DNS 解析全部地址；
3. 拒绝 loopback、私网、链路本地、保留、文档和组播地址；
4. 把已校验结果固定传给请求 lookup，防止 DNS 重绑定。

Node 20+ 可能以 `options.all = true` 调用 lookup。必须返回地址数组；只返回单地址会导致：

```text
Invalid IP address: undefined
```

限制：

- 图片 50 MB；
- 视频 1 GB；
- 图片边长 16384；
- 总像素 5000 万；
- 最多 5 次重定向；
- 校验 Content-Type、扩展名和文件头。

## 设置扩展流程

新增显示设置时同时修改：

1. `src/shared/contracts.ts`
   - `DisplaySettings`
   - `DEFAULT_SETTINGS`
2. `src-tauri/src/models.rs`
   - Rust 数据结构、默认值、patch 和 normalize
3. `src/renderer/App.tsx`
   - 控件、标签和预览变量
4. `src/main/payload.ts`
   - `ROOT_PROPERTIES`
   - `setProp()`
   - CSS 消费变量
5. 测试

透明度设置统一 clamp 到 0..1。不要让 UI 最小值和 normalize 最小值不一致。

## 已解决的典型故障

### Codex 显示未连接

原因曾是 `ApplicationActivationManager` CLSID 最后一位错误。

正确值：

```text
45ba127d-10a8-46ea-8ab7-56ea9078943c
```

关闭 Codex 时使用经过路径校验的进程，并允许进程已提前退出，避免 Stop-Process 竞态。

Tauri 迁移时还遇到过 Windows 上 `std::net::TcpStream::connect_timeout` 访问已监听的
Codex 回环 CDP 却返回 10060；同一端点通过 reqwest 可以立即连接。CDP 的
`/json/version` 和 `/json/list` 必须使用带总超时并禁用代理的 reqwest blocking
客户端，WebSocket 再交给 tungstenite。应用失败时也必须广播 controller 的错误状态，
否则界面会一直显示失败前的“尚未连接”。

启动 Studio 时要用 `runtime.json` 自动校验并恢复现有 browser ID；若状态失效则删除
旧状态，不能要求用户每次重开管理器都再次应用。状态文件丢失但 Codex 仍带调试参数时，
只允许从“可执行文件完整路径已匹配官方 Store 包”的进程命令行恢复调试端口。

Tauri 单实例插件必须是 Builder 注册的第一个插件。第二次启动只唤醒现有主窗口，
不能再创建第二套托盘、预览端口和 controller 状态。

### 页面冻结

原因：`install()` 每次无条件改写 style `textContent`，触发 MutationObserver，
形成无限微任务循环。

防护：

- style 保存 `data-cbg-revision`；
- revision 不同才写 textContent；
- CSS 变量值不同才 setProperty；
- DOM 更新按 requestAnimationFrame 合并。

### 页面黑屏或背景不显示

原因：Codex `app://` 页面不能稳定读取 Studio 回环媒体服务。

处理：base64 内嵌后转 Blob URL；设置 64 MB 上限；媒体 error 时 cleanup。

### 深色主题出现浅色界面

原因：依赖不存在的原生 CSS 变量，fallback 到浅灰。

处理：从根 class、data theme、computed color-scheme 和系统偏好检测主题，
设置 `--cbg-surface-color`。

### 左右区域同数值不同深浅

原因：`main.main-surface`、content viewport、right aside 多层半透明叠加，
加上 backdrop-filter。

处理：

- main 透明；
- content viewport 单层打底；
- right aside 单层打底；
- 内部壳透明；
- 关闭 backdrop-filter。

### Composer 黑色渐变

原因：独立 `bg-gradient-to-t` 元素和原生 shadow/border。

处理：清 gradient、background-image、shadow、border；composer 本身按独立变量打底。

### 站点、已安排、插件搜索黑条

原因：输入框透明，但外部 sticky 和 `::after` 仍有 main surface 与渐变。

处理：按共享结构清 sticky 与伪元素，不写三个页面专用补丁。

### 终端字符白底、选区黑底

原因：xterm ANSI `xterm-bg-*` 类和 selection layer 单独绘制背景。

处理：清字符背景和 selection div；恢复反相字符前景色。

### 审阅滚动时先黑后透明

原因有两层：

- diff 使用 Shadow DOM；普通 CSS 无法穿透，旧实现等 MutationObserver 约 200ms 后注入；
- `diffs-container` 在真实 `[data-diff]` 出现前，会先在 Shadow `:host` 写入
  `background-color: #111111` 和 `--diffs-bg: #111111`。只覆盖 `[data-diff]`
  会让虚拟滚动的新代码块先按这个宿主默认值绘制，再在内容节点挂载后变透明。

处理：

- 早期 payload 在 documentElement 阶段运行；
- 在普通文档 CSS 的 `diffs-container` 宿主上预先固定透明 surface、context、
  separator、hover 和 `--diffs-bg` 变量，让占位阶段直接继承透明值；
- 根级包装 `Element.prototype.attachShadow`；
- Shadow Root 创建同一帧注入，并在 Shadow CSS 的 `:host` 再固定同一组变量；
- 微任务和 requestAnimationFrame 各确保一次样式位于末尾；
- cleanup 恢复原方法；
- timer 仅兜底。

不要回退到“出现后延迟覆盖”的实现。

## 调试策略

### 推荐

- 先 `npm run dev` 调试 Tauri Studio。
- 只看 Studio UI 时用 `npm run dev:web`。
- 用一次性 Node `.mjs` 或忽略的 Rust live test 连接 Codex CDP。
- 把 DOM、计算样式和截图作为证据。
- 对动态页面测试导航后、滚动后、重载后状态。

### 避免

- 不用类名关键词大范围 `background: transparent`。
- 不根据截图颜色猜元素。
- 不依赖随机生成的 CSS module 类。
- 不用 `setInterval` 作为正常首帧方案。
- 不用 opacity 作用于整个代码组件；这会连文字一起淡化。
- 不隐藏用户仍需交互的原生提示，仅调整其表面透明度。

## 测试和发布

`npm run check` 包含：

- renderer TypeScript；
- main/preload TypeScript；
- Vitest。

payload 测试至少应断言：

- 关键稳定选择器仍存在；
- Shadow style id 和 diff 变量存在；
- `attachShadow`、`requestAnimationFrame` 首帧方案存在；
- 不再出现 200ms 固定延迟；
- 不引入 backdrop blur；
- 不残留参考项目私有标记。

发布前：

1. 删除 `poc/` 一次性文件。
2. 跑 `npm run check`。
3. 在真实 Codex 完成页面矩阵验证。
4. 更新 package 与 lock 版本。
5. 跑 `npm run package:win`。
6. 检查安装包大小和 SHA256。
7. 查看 Git 完整 diff。
8. 用户明确同意后提交、推送。
