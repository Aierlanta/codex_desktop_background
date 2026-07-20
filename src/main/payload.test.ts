import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "../shared/contracts.js";
import { buildRendererPayload, earlyPayloadFor, REMOVE_RENDERER_PAYLOAD } from "./payload.js";

describe("renderer payload", () => {
  it("contains route-aware background controls and an inert decorative layer", () => {
    const payload = buildRendererPayload({
      mediaUrl: "http://127.0.0.1:9444/token/media/id",
      mediaKind: "video",
      display: DEFAULT_SETTINGS.display,
      revision: "revision-1",
    });
    expect(payload).toContain("codex-background-layer");
    expect(payload).toContain("pointer-events: none");
    expect(payload).toContain("codex-background-home");
    expect(payload).toContain("codex-background-task");
    expect(payload).toContain("media.playbackRate");
    expect(payload).toContain(".app-shell-main-content-viewport");
    expect(payload).toContain(".home-banners");
    expect(payload).toContain('[class~=\\"sticky\\"][class*=\\"bg-token-main-surface-primary\\"]:has(input[type=\\"text\\"])');
    expect(payload).toContain('[class~=\\"h-full\\"][class~=\\"min-h-0\\"][class~=\\"flex-col\\"]');
    expect(payload).toContain('aside[class~=\\"ml-auto\\"]');
    expect(payload).toContain('aside[class~=\\"ml-auto\\"][class*=\\"z-[41]\\"] [class*=\\"bg-token-main-surface-primary\\"]');
    expect(payload).not.toContain(':has(ul button[class*=\\"bg-token-bg-fog\\"])');
    expect(payload).not.toContain("backdrop-filter: blur");
    expect(payload).not.toContain("__DREAM_");
  });

  it("serializes media URLs instead of interpolating executable source", () => {
    const payload = buildRendererPayload({
      mediaUrl: "http://127.0.0.1/media/\";window.pwned=true;//",
      mediaKind: "image",
      display: DEFAULT_SETTINGS.display,
      revision: "safe",
    });
    expect(payload).toContain("\\\"");
    expect(payload).not.toContain('mediaUrl: "http');
  });

  it("provides early-document installation and reversible removal", () => {
    const early = earlyPayloadFor("(() => true)()", "abc");
    expect(early).toContain("MutationObserver");
    expect(early).toContain("abc");
    expect(REMOVE_RENDERER_PAYLOAD).toContain("cleanup");
  });
});

