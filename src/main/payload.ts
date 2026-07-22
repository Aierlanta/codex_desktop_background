import { createHash } from "node:crypto";
import { DisplaySettings, MediaKind } from "../shared/contracts.js";

const BACKGROUND_CSS = String.raw`
html.codex-background-active,
html.codex-background-active body {
  background: transparent !important;
}

html.codex-background-active body > :not(#codex-background-layer) {
  position: relative;
  z-index: 1;
}

#codex-background-layer {
  position: fixed;
  inset: 0;
  z-index: 0;
  overflow: hidden;
  pointer-events: none;
  opacity: calc(var(--cbg-opacity) * var(--cbg-route-intensity));
  background-color: #101416;
  transition: opacity 220ms ease;
  contain: strict;
}

#codex-background-media,
#codex-background-tile {
  width: 100%;
  height: 100%;
  transform: scale(var(--cbg-scale));
  filter: blur(var(--cbg-blur));
  transform-origin: center;
}

#codex-background-media {
  display: block;
  object-fit: var(--cbg-fit);
  object-position: var(--cbg-position-x) var(--cbg-position-y);
}

#codex-background-tile {
  display: none;
  background-image: var(--cbg-media-url);
  background-repeat: repeat;
  background-position: var(--cbg-position-x) var(--cbg-position-y);
}

html.codex-background-fit-tile #codex-background-media { display: none; }
html.codex-background-fit-tile #codex-background-tile { display: block; }

#codex-background-overlay {
  position: absolute;
  inset: 0;
  background: var(--cbg-overlay-color);
  opacity: var(--cbg-overlay-opacity);
}

html.codex-background-home { --cbg-route-intensity: var(--cbg-home-intensity); }
html.codex-background-task { --cbg-route-intensity: var(--cbg-task-intensity); }
html.codex-background-home.codex-background-home-disabled,
html.codex-background-task.codex-background-task-disabled { --cbg-route-intensity: 0; }

html.codex-background-active aside.app-shell-left-panel,
html.codex-background-active div[class~="fixed"][class~="left-0"][class~="z-[42]"][class*="top-(--height-toolbar-sm)"] > aside[class*="bg-token-main-surface-primary"] {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-sidebar-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}

html.codex-background-active aside.app-shell-left-panel nav,
html.codex-background-active div[class~="fixed"][class~="left-0"][class~="z-[42]"][class*="top-(--height-toolbar-sm)"] > aside[class*="bg-token-main-surface-primary"] nav,
html.codex-background-active main.main-surface > header.app-header-tint,
html.codex-background-active main.main-surface .app-shell-main-content-top-fade {
  background: transparent !important;
}

html.codex-background-active main.main-surface {
  background: transparent !important;
  backdrop-filter: none !important;
}

html.codex-background-active main.main-surface .app-shell-main-content-viewport {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-surface-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}

html.codex-background-active main.main-surface [role="main"],
html.codex-background-active main.main-surface .app-shell-main-content-frame,
html.codex-background-active main.main-surface [class~="bg-token-main-surface-primary"][class~="h-full"][class~="w-full"] {
  background: transparent !important;
}
html.codex-background-active main.main-surface .app-shell-main-content-viewport [class~="h-full"][class~="min-h-0"][class~="flex-col"][class*="bg-token-main-surface-primary"] {
  background-color: transparent !important;
}

html.codex-background-active .composer-surface-chrome,
html.codex-background-active main.main-surface div.no-drag:has(> input[type="text"]),
html.codex-background-active main.main-surface div.no-drag:has(> textarea) {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-composer-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
  border-color: transparent !important;
}
html.codex-background-active main.main-surface [class*="bg-gradient-to-t"][class*="from-token-main-surface-primary"][class*="via-token-main-surface-primary"] {
  background-color: transparent !important;
  background-image: none !important;
}
html.codex-background-active main.main-surface .app-shell-main-content-viewport [class~="sticky"][class*="bg-token-main-surface-primary"]:has(input[type="text"]),
html.codex-background-active main.main-surface .app-shell-main-content-viewport [class~="sticky"][class*="bg-token-main-surface-primary"]:has(input[type="text"])::after {
  background-color: transparent !important;
  background-image: none !important;
}

/* 窗口级应用菜单栏：独立于 main.main-surface，原生依赖下层不透明底色，
   需要单独给表面色打底，否则背景图会从这条全宽栏里直接透出来 */
html.codex-background-active [class~="app-header-tint"][class*="application-menu-top-bar"] {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-surface-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}

/* 弹出层：下拉菜单、右键上下文菜单、命令面板等（portal 渲染，原生为不透明实底）。
   统一按菜单不透明度打底并模糊，让背景图可控地透出 */
html.codex-background-active [role="menu"],
html.codex-background-active [role="listbox"],
html.codex-background-active [class*="bg-token-dropdown-background"]:not(.composer-surface-chrome) {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}
/* 菜单内部的分组标题/子层继承透明，避免叠加出不透明色块 */
html.codex-background-active [class*="bg-token-dropdown-background"]:not(.composer-surface-chrome) [class*="bg-token-dropdown-background"] {
  background-color: transparent !important;
  backdrop-filter: none !important;
}

/* 首页推荐横幅（例如“启用快速模式”）是独立于输入栏的原生卡片。
   让卡片跟随菜单/右侧面板不透明度，避免固定实底悬在透明首页上。 */
html.codex-background-active main.main-surface .home-banners > aside[class*="bg-token-main-surface-primary"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}

/* 任务页右侧辅助栏的内容会在任务/浏览器/终端间切换，不能依赖内部按钮识别。
   使用稳定的右侧 aside 容器统一打底，并清掉所有内容页自带的 main-surface 实底。 */
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] [class*="bg-token-main-surface-primary"] {
  background-color: transparent !important;
}
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] .codex-review-diff-card,
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] diffs-container,
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] file-tree-container {
  background-color: transparent !important;
  background-image: none !important;
  --color-token-main-surface-primary: transparent !important;
}
/* diffs-container 渲染真实 [data-diff] 前，会先在 Shadow :host 写入 #111111。
   把所有底色变量直接固定在宿主上，让占位、虚拟滚动和正式内容从首帧起就继承透明值。 */
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] diffs-container {
  --codex-diffs-surface: transparent !important;
  --codex-diffs-context-surface: transparent !important;
  --codex-diffs-separator-surface: transparent !important;
  --codex-diffs-hover-surface: transparent !important;
  --codex-diffs-header-surface: transparent !important;
  --diffs-bg: transparent !important;
  --diffs-bg-context-override: transparent !important;
  --diffs-bg-separator-override: transparent !important;
  --diffs-bg-hover-override: transparent !important;
  --diffs-bg-addition: color-mix(in srgb, var(--diffs-addition-base, #40c977) 12%, transparent) !important;
  --diffs-bg-deletion: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 12%, transparent) !important;
  --codex-diffs-addition-number: color-mix(in srgb, var(--diffs-addition-base, #40c977) 20%, transparent) !important;
  --codex-diffs-deletion-number: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 20%, transparent) !important;
}
html.codex-background-active main.main-surface aside[class~="ml-auto"][class*="z-[41]"] .codex-review-diff-card > [class~="sticky"][class~="backdrop-blur-sm"] {
  background-color: transparent !important;
  backdrop-filter: none !important;
}

/* 集成终端面板：原生为多层嵌套的不透明实底(bg-token-main-surface-primary)。
   先把所有包裹层设为透明，避免多层半透明叠加，再只给终端面板本体打一层
   按内容区不透明度控制的底色并模糊，让背景可控透出、终端文字仍清晰。 */
html.codex-background-active div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"]) {
  background-color: transparent !important;
}
html.codex-background-active [id^="terminal-panel-"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-terminal-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}
html.codex-background-active div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"]) > [class~="h-toolbar-pane"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-terminal-opacity) * 100%), transparent) !important;
}
html.codex-background-active div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"]) > [class~="h-toolbar-pane"] * {
  background-color: transparent !important;
  background-image: none !important;
}
/* xterm 会用 ANSI 背景类绘制白底/黑底字符块。背景工具模式下清掉字符底色，
   对反相文本恢复终端前景色；选区保留轻量半透明提示而不使用黑色实底。 */
html.codex-background-active [id^="terminal-panel-"] .xterm-rows span[class*="xterm-bg-"] {
  background-color: transparent !important;
}
html.codex-background-active [id^="terminal-panel-"] .xterm-rows .xterm-bg-257.xterm-fg-257 {
  color: var(--vscode-terminal-foreground, var(--color-text-foreground, #fff)) !important;
}
html.codex-background-active [id^="terminal-panel-"] .xterm-selection > div,
html.codex-background-active [id^="terminal-panel-"] .xterm-selection-layer > div {
  background-color: transparent !important;
}

html.codex-background-dark #codex-background-layer {
  background-color: #0d0f12;
}

@media (prefers-reduced-motion: reduce) {
  #codex-background-layer { transition: none; }
}
`;

const REVIEW_SHADOW_STYLE_ID = "codex-background-review-shadow-style";
const REVIEW_SHADOW_CSS = String.raw`
:host,
[data-diffs-header],
:is([data-diff], [data-file]) {
  --color-token-main-surface-primary: transparent !important;
  --codex-diffs-surface: transparent !important;
  --codex-diffs-context-surface: transparent !important;
  --codex-diffs-separator-surface: transparent !important;
  --codex-diffs-hover-surface: transparent !important;
  --codex-diffs-header-surface: transparent !important;
  --diffs-bg: transparent !important;
  --diffs-bg-context-override: transparent !important;
  --diffs-bg-separator-override: transparent !important;
  --diffs-bg-hover-override: transparent !important;
  --diffs-bg-addition: color-mix(in srgb, var(--diffs-addition-base, #40c977) 12%, transparent) !important;
  --diffs-bg-deletion: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 12%, transparent) !important;
  --codex-diffs-addition-number: color-mix(in srgb, var(--diffs-addition-base, #40c977) 20%, transparent) !important;
  --codex-diffs-deletion-number: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 20%, transparent) !important;
  background-color: transparent !important;
}
`;

export interface PayloadInput {
  mediaUrl: string;
  mediaKind: MediaKind;
  display: DisplaySettings;
  revision: string;
}

export function buildRendererPayload(input: PayloadInput) {
  // 修订号混入 CSS 内容哈希：工具升级改了注入样式后，
  // 即使媒体和显示设置没变也要强制重写页面里的 <style>
  const revision = createHash("sha256")
    .update(input.revision)
    .update(BACKGROUND_CSS)
    .update(REVIEW_SHADOW_CSS)
    .digest("hex");
  const serialized = JSON.stringify({ ...input, revision }).replace(/</g, "\\u003c");
  const css = JSON.stringify(BACKGROUND_CSS);
  const reviewShadowCss = JSON.stringify(REVIEW_SHADOW_CSS);
  const reviewShadowStyleId = JSON.stringify(REVIEW_SHADOW_STYLE_ID);
  return String.raw`((config, cssText, reviewShadowCssText, reviewShadowStyleId) => {
    const STATE = "__CODEX_BACKGROUND_STUDIO__";
    const STYLE_ID = "codex-background-style";
    const LAYER_ID = "codex-background-layer";
    const REVIEW_HOST_SELECTOR = "diffs-container";
    const ROOT_CLASSES = [
      "codex-background-active", "codex-background-home", "codex-background-task",
      "codex-background-home-disabled", "codex-background-task-disabled",
      "codex-background-fit-tile", "codex-background-dark"
    ];
    const ROOT_PROPERTIES = [
      "--cbg-opacity", "--cbg-blur", "--cbg-scale", "--cbg-fit",
      "--cbg-position-x", "--cbg-position-y", "--cbg-overlay-color",
      "--cbg-overlay-opacity", "--cbg-home-intensity", "--cbg-task-intensity",
      "--cbg-route-intensity", "--cbg-sidebar-opacity", "--cbg-surface-opacity",
      "--cbg-composer-opacity", "--cbg-menu-opacity", "--cbg-terminal-opacity",
      "--cbg-media-url", "--cbg-surface-color"
    ];

    const previous = window[STATE];
    if (previous?.cleanup) {
      previous.cleanup();
    } else {
      if (previous?.observer) previous.observer.disconnect();
      if (previous?.timer) clearInterval(previous.timer);
      previous?.layer?.remove();
      if (previous?.blobUrl) URL.revokeObjectURL(previous.blobUrl);
    }
    let scheduled = null;
    let shadowPatch = null;

    // Codex 渲染页无法访问本机 HTTP 服务，媒体以 base64 内嵌传入，
    // 在页面内转成 Blob URL 使用
    const blobUrl = (() => {
      const comma = config.mediaUrl.indexOf(",");
      if (!config.mediaUrl.startsWith("data:") || comma < 0) return config.mediaUrl;
      const binary = atob(config.mediaUrl.slice(comma + 1));
      const bytes = new Uint8Array(binary.length);
      for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
      const mime = /^data:([^;,]+)/.exec(config.mediaUrl)?.[1] || "application/octet-stream";
      return URL.createObjectURL(new Blob([bytes], { type: mime }));
    })();

    const installReviewShadowStyle = (host, shadow = host?.shadowRoot) => {
      if (!shadow) return false;
      let shadowStyle = shadow.getElementById(reviewShadowStyleId);
      if (!shadowStyle) {
        shadowStyle = document.createElement("style");
        shadowStyle.id = reviewShadowStyleId;
      }
      if (shadowStyle.dataset.cbgRevision !== config.revision) {
        shadowStyle.textContent = reviewShadowCssText;
        shadowStyle.dataset.cbgRevision = config.revision;
      }
      // 始终放到 Shadow DOM 样式末尾，确保覆盖组件稍后同步追加的原生样式。
      shadow.appendChild(shadowStyle);
      return true;
    };

    const cleanup = () => {
      const state = window[STATE];
      state?.observer?.disconnect();
      if (state?.timer) clearInterval(state.timer);
      if (scheduled) cancelAnimationFrame(scheduled);
      if (shadowPatch?.prototype.attachShadow === shadowPatch.wrapped) {
        shadowPatch.prototype.attachShadow = shadowPatch.original;
      }
      document.getElementById(LAYER_ID)?.remove();
      document.getElementById(STYLE_ID)?.remove();
      document.querySelectorAll("diffs-container").forEach((host) => {
        host.shadowRoot?.getElementById(reviewShadowStyleId)?.remove();
      });
      document.documentElement?.classList.remove(...ROOT_CLASSES);
      for (const property of ROOT_PROPERTIES) document.documentElement?.style.removeProperty(property);
      if (state?.blobUrl) URL.revokeObjectURL(state.blobUrl);
      delete window[STATE];
      return true;
    };

    // 页面创建 diff Shadow DOM 时同步接管；微任务和下一帧各补一次，
    // 保证组件无论同步还是异步追加原生样式，我们的覆盖都在首帧绘制前位于末尾。
    const patchAttachShadow = () => {
      const prototype = Element.prototype;
      const original = prototype.attachShadow;
      const wrapped = function(init) {
        const shadow = original.call(this, init);
        if (this.localName === REVIEW_HOST_SELECTOR) {
          queueMicrotask(() => installReviewShadowStyle(this, shadow));
          requestAnimationFrame(() => installReviewShadowStyle(this, shadow));
        }
        return shadow;
      };
      prototype.attachShadow = wrapped;
      return { prototype, original, wrapped };
    };
    shadowPatch = patchAttachShadow();

    // 检测 Codex 原生外观：优先读根节点/滚动容器的计算 color-scheme
    //（跟随应用内主题设置），系统偏好只作兜底
    const detectAppearance = () => {
      const root = document.documentElement;
      const classText = ((root?.className || "") + " " + (document.body?.className || ""))
        .toLowerCase()
        .replace(/\bcodex-background-[a-z-]+\b/g, "");
      if (/\b(?:dark|theme-dark|appearance-dark)\b/.test(classText)) return "dark";
      if (/\b(?:light|theme-light|appearance-light)\b/.test(classText)) return "light";
      const dataTheme = (
        root?.getAttribute("data-theme") || root?.getAttribute("data-appearance") ||
        document.body?.getAttribute("data-theme") || ""
      ).toLowerCase();
      if (dataTheme.includes("dark")) return "dark";
      if (dataTheme.includes("light")) return "light";
      try {
        const scheme = getComputedStyle(root).colorScheme || "";
        if (scheme.includes("dark") && !scheme.includes("light")) return "dark";
        if (scheme.includes("light") && !scheme.includes("dark")) return "light";
      } catch {}
      try {
        return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      } catch {}
      return "light";
    };

    const install = () => {
      const root = document.documentElement;
      if (!root) return false;

      // 值不同才写入，避免 attribute 观察被自己的写入反复触发
      const setClass = (name, on) => {
        if (root.classList.contains(name) !== on) root.classList.toggle(name, on);
      };
      const setProp = (name, value) => {
        if (root.style.getPropertyValue(name) !== value) root.style.setProperty(name, value);
      };

      const dark = detectAppearance() === "dark";
      setClass("codex-background-dark", dark);
      setProp("--cbg-surface-color", dark ? "#16181c" : "#f6f7f7");

      let style = document.getElementById(STYLE_ID);
      if (!style) {
        style = document.createElement("style");
        style.id = STYLE_ID;
        (document.head || root).appendChild(style);
      }
      // 只在修订号变化时写入样式：无条件写 textContent 会触发 childList
      // 变更，被下面的 MutationObserver 观察到后再次进入 install()，
      // 形成微任务死循环，直接卡死渲染进程
      if (style.dataset.cbgRevision !== config.revision) {
        style.textContent = cssText;
        style.dataset.cbgRevision = config.revision;
      }
      // 审阅 diff 使用 Shadow DOM，普通页面 CSS 无法进入其内部。
      // 对每个已挂载的 diff 宿主注入同一份轻量样式；定时 install 会覆盖后续新建的宿主。
      document.querySelectorAll(
        'main.main-surface aside[class~="ml-auto"][class*="z-[41]"] diffs-container'
      ).forEach((host) => {
        installReviewShadowStyle(host);
      });

      let layer = document.getElementById(LAYER_ID);
      if (!layer && document.body) {
        layer = document.createElement("div");
        layer.id = LAYER_ID;
        const media = document.createElement(config.mediaKind === "video" ? "video" : "img");
        media.id = "codex-background-media";
        media.setAttribute("aria-hidden", "true");
        if (config.mediaKind === "video") {
          media.autoplay = true;
          media.loop = true;
          media.muted = Boolean(config.display.videoMuted);
          media.defaultMuted = Boolean(config.display.videoMuted);
          media.playsInline = true;
          media.playbackRate = Number(config.display.videoPlaybackRate) || 1;
        }
        media.src = blobUrl;
        // 媒体加载失败时整体回退到原生外观，避免留下深色空背景层遮挡界面
        media.addEventListener("error", () => cleanup());
        const tile = document.createElement("div");
        tile.id = "codex-background-tile";
        const overlay = document.createElement("div");
        overlay.id = "codex-background-overlay";
        layer.append(media, tile, overlay);
        document.body.prepend(layer);
        if (config.mediaKind === "video") media.play().catch(() => undefined);
      }

      setClass("codex-background-active", true);
      setClass("codex-background-fit-tile", config.display.fit === "tile" && config.mediaKind === "image");
      setClass("codex-background-home-disabled", !config.display.enabledOnHome);
      setClass("codex-background-task-disabled", !config.display.enabledOnTasks);
      setProp("--cbg-opacity", String(config.display.opacity));
      setProp("--cbg-blur", config.display.blur + "px");
      setProp("--cbg-scale", String(config.display.scale));
      setProp("--cbg-fit", config.display.fit === "tile" ? "cover" : config.display.fit);
      setProp("--cbg-position-x", config.display.positionX + "%");
      setProp("--cbg-position-y", config.display.positionY + "%");
      setProp("--cbg-overlay-color", config.display.overlayColor);
      setProp("--cbg-overlay-opacity", String(config.display.overlayOpacity));
      setProp("--cbg-home-intensity", String(config.display.homeIntensity));
      setProp("--cbg-task-intensity", String(config.display.taskIntensity));
      setProp("--cbg-sidebar-opacity", String(config.display.sidebarOpacity));
      setProp("--cbg-surface-opacity", String(config.display.surfaceOpacity));
      setProp("--cbg-composer-opacity", String(config.display.composerOpacity));
      setProp("--cbg-menu-opacity", String(config.display.menuOpacity));
      setProp("--cbg-terminal-opacity", String(config.display.terminalOpacity));
      setProp("--cbg-media-url", 'url("' + String(blobUrl).replace(/["\\\n\r]/g, "") + '")');

      const home = Boolean(document.querySelector('[role="main"]:has([data-testid="home-icon"])'));
      setClass("codex-background-home", home);
      setClass("codex-background-task", !home);
      return true;
    };

    const scheduleInstall = () => {
      if (scheduled) return;
      scheduled = requestAnimationFrame(() => { scheduled = null; install(); });
    };
    const observer = new MutationObserver(scheduleInstall);
    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["class", "data-theme", "data-appearance"],
    });
    const timer = setInterval(install, 4000);
    window[STATE] = { revision: config.revision, cleanup, observer, timer, layer: null, blobUrl };
    install();
    window[STATE].layer = document.getElementById(LAYER_ID);
    return { installed: true, revision: config.revision, mediaKind: config.mediaKind };
  })(${serialized}, ${css}, ${reviewShadowCss}, ${reviewShadowStyleId})`;
}

export const REMOVE_RENDERER_PAYLOAD = String.raw`(() => {
  const state = window.__CODEX_BACKGROUND_STUDIO__;
  if (state?.cleanup) return state.cleanup();
  document.getElementById("codex-background-layer")?.remove();
  document.getElementById("codex-background-style")?.remove();
  document.documentElement?.classList.remove(
    "codex-background-active", "codex-background-home", "codex-background-task",
    "codex-background-home-disabled", "codex-background-task-disabled", "codex-background-fit-tile"
  );
  delete window.__CODEX_BACKGROUND_STUDIO__;
  return true;
})()`;

export function earlyPayloadFor(payload: string, revision: string) {
  const safeRevision = JSON.stringify(revision);
  return String.raw`(() => {
    const revision = ${safeRevision};
    const run = () => {
      if (!document.documentElement) return false;
      try { ${payload}; return true; } catch { return false; }
    };
    if (!run()) {
      const observer = new MutationObserver(() => {
        if (run()) observer.disconnect();
      });
      observer.observe(document.documentElement || document, { childList: true, subtree: true });
      setTimeout(() => observer.disconnect(), 30000);
    }
    return revision;
  })()`;
}

