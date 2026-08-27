/*
 * CogniSwitch brand theming for Mermaid diagrams.
 *
 * mkdocs-material's own Mermaid integration calls
 *   mermaid.initialize({ startOnLoad: false, themeCSS: <var()-based overlay>, ... })
 * without ever passing an explicit `theme`, so Mermaid falls back to its
 * built-in 'default' theme (hardcoded pale-lavender node fills) and only
 * *overlays* a thin CSS string that references our --md-mermaid-* variables.
 * Mermaid renders each diagram inside a closed shadow root
 * (attachShadow({mode:"closed"})), so nothing outside can repaint it once
 * drawn, and in practice that var()-based overlay was losing to Mermaid's
 * own generated theme styles for flowchart node fills/text -- diagrams
 * rendered with light, low-contrast colors regardless of dark mode.
 *
 * Fix: monkey-patch mermaid.initialize (however/whenever it gets called --
 * by us or by mkdocs-material) so it always merges in theme: 'base' plus a
 * full themeVariables object built from the *actual* computed CogniSwitch
 * brand CSS variables (docs/stylesheets/brand.css) at call time. Mermaid's
 * 'base' theme bakes these as literal colors into the generated SVG, which
 * sidesteps the unreliable var()-in-closed-shadow-root overlay entirely.
 */
(function () {
  "use strict";

  function readVar(name, fallback) {
    var value = getComputedStyle(document.body).getPropertyValue(name).trim();
    return value || fallback;
  }

  function brandThemeVariables() {
    // Mermaid's 'base' theme derives many other colors from primaryColor
    // via internal lighten/darken color math, which doesn't play well with
    // a low-alpha rgba() (our --md-mermaid-node-bg-color is deliberately
    // translucent for the var()-overlay path). Use the opaque code/card
    // surface color for node fills here instead -- solid, readable, and
    // safe for mermaid's own color computations.
    var nodeBg = readVar("--md-code-bg-color", "#e5e5e0");
    var nodeFg = readVar("--md-mermaid-node-fg-color", "#0808ba");
    var edge = readVar("--md-mermaid-edge-color", "#272048");
    var labelBg = readVar("--md-mermaid-label-bg-color", "#f4f4f0");
    var labelFg = readVar("--md-mermaid-label-fg-color", "#272048");
    var pageBg = readVar("--md-default-bg-color", "#f4f4f0");
    var clusterBorder = readVar("--md-default-fg-color--lighter", "#9ca3af");
    var fontFamily = readVar("--font-inter", "Inter, -apple-system, sans-serif");

    return {
      background: pageBg,
      fontFamily: fontFamily,

      primaryColor: nodeBg,
      primaryTextColor: nodeFg,
      primaryBorderColor: nodeFg,

      secondaryColor: labelBg,
      secondaryTextColor: labelFg,
      secondaryBorderColor: clusterBorder,

      tertiaryColor: pageBg,
      tertiaryTextColor: labelFg,
      tertiaryBorderColor: clusterBorder,

      lineColor: edge,
      textColor: labelFg,

      mainBkg: nodeBg,
      nodeBorder: nodeFg,
      nodeTextColor: nodeFg,

      clusterBkg: pageBg,
      clusterBorder: clusterBorder,

      defaultLinkColor: edge,
      titleColor: labelFg,
      edgeLabelBackground: labelBg,

      actorBkg: labelBg,
      actorBorder: nodeFg,
      actorTextColor: labelFg,
      actorLineColor: clusterBorder,

      noteBkgColor: labelBg,
      noteTextColor: labelFg,
      noteBorderColor: labelFg,
    };
  }

  function forceBrandTheme(config) {
    config = config || {};
    config.theme = "base";
    config.themeVariables = Object.assign(
      {},
      brandThemeVariables(),
      config.themeVariables || {}
    );
    return config;
  }

  function installHook(instance) {
    if (!instance || instance.__cogniswitchThemed) {
      return;
    }
    instance.__cogniswitchThemed = true;
    var originalInitialize = instance.initialize.bind(instance);
    instance.initialize = function (config) {
      return originalInitialize(forceBrandTheme(config));
    };
  }

  if (window.mermaid) {
    installHook(window.mermaid);
  } else {
    var current;
    Object.defineProperty(window, "mermaid", {
      configurable: true,
      get: function () {
        return current;
      },
      set: function (value) {
        current = value;
        installHook(value);
      },
    });
  }

  // Mermaid diagrams are rendered once (lazily, on first use) into closed
  // shadow roots that nothing outside can repaint after the fact, so a
  // live light/dark toggle can't be reflected in already-rendered
  // diagrams. Reload on scheme change so the next render picks up the
  // correct scheme's brand colors from scratch.
  var lastScheme = document.body.getAttribute("data-md-color-scheme");
  new MutationObserver(function () {
    var scheme = document.body.getAttribute("data-md-color-scheme");
    if (scheme !== lastScheme && document.querySelector(".mermaid")) {
      location.reload();
    }
    lastScheme = scheme;
  }).observe(document.body, {
    attributes: true,
    attributeFilter: ["data-md-color-scheme"],
  });
})();
