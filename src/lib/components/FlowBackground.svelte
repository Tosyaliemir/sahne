<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { palette } from "../stores/palette.svelte";
  import { player } from "../stores/player.svelte";
  import { ui } from "../stores/ui.svelte";
  import { settings } from "../stores/settings.svelte";
  import { frames } from "../frames";

  /**
   * Fluid backdrop: the album art itself, in two layers turning in
   * opposite directions and bent by a slowly moving noise field, blended
   * with the ambient palette and finished with fine grain against banding.
   * Rendered at reduced resolution and scaled up (it is soft by design).
   *
   * It moves only while music plays, at a capped frame rate (the motion is
   * slow), and draws nothing once paused and its colours have settled.
   * Calls `onUnsupported` if WebGL is unavailable, or rendering is software
   * only (unless the performance mode is "quality"), so the caller can fall
   * back to the CSS backdrop.
   */
  let { onUnsupported }: { onUnsupported: () => void } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();

  const MAX_WIDTH = 960;
  const TEX_SIZE = 256;
  const ART_FADE_MS = 1400;
  const COLOR_EASE_MS = 900;
  /** Frame rate and render scale per performance mode. */
  const MODES = {
    quality: { fps: 60, scale: 0.4 },
    auto: { fps: 30, scale: 0.4 },
    battery: { fps: 20, scale: 0.25 },
  } as const;
  /** Auto mode drops to this scale when frames keep arriving late. */
  const AUTO_LOW_SCALE = 0.3;
  const SOFTWARE_RENDERER = /llvmpipe|softpipe|swiftshader|software/i;

  const VERT = `
    attribute vec2 aPos;
    varying vec2 vUv;
    void main() {
      vUv = aPos * 0.5 + 0.5;
      gl_Position = vec4(aPos, 0.0, 1.0);
    }`;

  // highp matters: with real 16-bit mediump (e.g. NVIDIA under WebKitGTK) the
  // hash/noise math collapses and only part of the screen gets any pattern.
  const FRAG = `
    #ifdef GL_FRAGMENT_PRECISION_HIGH
    precision highp float;
    #else
    precision mediump float;
    #endif
    varying vec2 vUv;
    uniform sampler2D uArtA;
    uniform sampler2D uArtB;
    uniform float uMix;      // 0 = A, 1 = B (cross-fade between covers)
    uniform float uHasArt;
    uniform float uTime;
    uniform vec2 uRes;
    uniform vec3 uAmb[5];
    uniform vec3 uBg;
    uniform float uDim;

    float hash(vec2 p) {
      p = fract(p * vec2(123.34, 456.21));
      p += dot(p, p + 45.32);
      return fract(p.x * p.y);
    }
    float noise(vec2 p) {
      vec2 i = floor(p);
      vec2 f = fract(p);
      vec2 u = f * f * (3.0 - 2.0 * f);
      return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
                 mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
    }
    float fbm(vec2 p) {
      float v = 0.0;
      float a = 0.5;
      for (int i = 0; i < 4; i++) {
        v += a * noise(p);
        p = p * 2.03 + vec2(17.1, 9.7);
        a *= 0.5;
      }
      return v;
    }
    mat2 rot(float a) {
      float c = cos(a);
      float s = sin(a);
      return mat2(c, -s, s, c);
    }
    // Mirror-repeat without needing REPEAT on the texture.
    vec2 mirror(vec2 uv) {
      return 1.0 - abs(1.0 - mod(uv, 2.0));
    }
    vec3 art(vec2 uv) {
      // Bias toward lower mip levels: soft, but keeps the cover's shapes.
      vec3 a = texture2D(uArtA, mirror(uv), 2.0).rgb;
      vec3 b = texture2D(uArtB, mirror(uv), 2.0).rgb;
      return mix(a, b, uMix);
    }
    vec3 ambient(float k) {
      k = fract(k) * 5.0;
      vec3 c = uAmb[0];
      c = mix(c, uAmb[1], smoothstep(0.0, 1.0, k));
      c = mix(c, uAmb[2], smoothstep(1.0, 2.0, k));
      c = mix(c, uAmb[3], smoothstep(2.0, 3.0, k));
      c = mix(c, uAmb[4], smoothstep(3.0, 4.0, k));
      c = mix(c, uAmb[0], smoothstep(4.0, 5.0, k));
      return c;
    }

    void main() {
      float aspect = uRes.x / uRes.y;
      vec2 p = vUv - 0.5;
      p.x *= aspect;
      float t = uTime * 0.035;

      // Domain warping: two noise passes bend the plane like slow liquid.
      vec2 q = vec2(fbm(p * 1.3 + t), fbm(p * 1.3 - t + 5.2));
      vec2 r = vec2(fbm(p * 1.6 + 2.2 * q + vec2(1.7, 9.2) + t * 1.2),
                    fbm(p * 1.6 + 2.2 * q + vec2(8.3, 2.8) - t * 0.9));

      // Two copies of the cover, turning opposite ways at different scales.
      vec2 a = rot(t * 0.9) * p * 0.85 + (r - 0.5) * 1.1;
      vec2 b = rot(-t * 0.6 + 1.3) * p * 1.35 + (q - 0.5) * 0.9 + vec2(0.31, -0.17);
      a.x /= aspect;
      b.x /= aspect;
      vec3 cover = mix(art(a + 0.5), art(b + 0.5), smoothstep(0.3, 0.7, r.x));

      vec3 amb = ambient(fbm(p * 0.9 + r * 1.4 + t * 0.4) * 1.6 + t * 0.15);
      vec3 col = mix(amb, cover, 0.62 * uHasArt);
      // Mostly-black covers would leave a dead backdrop; let the palette glow through.
      col = max(col, amb * 0.42);

      // A little extra saturation; then tame highlights and deepen toward the
      // background color so light lyrics always stay readable on top.
      float luma = dot(col, vec3(0.299, 0.587, 0.114));
      col = mix(vec3(luma), col, 1.12);
      col /= 1.0 + luma * 0.8;
      float vignette = smoothstep(1.3, 0.2, length(p));
      col = mix(uBg, col, vignette * mix(0.85, 0.25, uDim));

      // Fine grain hides 8-bit banding in the smooth gradients.
      col += (hash(gl_FragCoord.xy + fract(uTime) * 61.0) - 0.5) * (2.0 / 255.0);
      gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
    }`;

  function report(message: string) {
    invoke("frontend_log", { level: "info", message }).catch(() => {});
  }

  function writeRgb(hex: string, out: Float32Array, at: number): void {
    const v = parseInt(hex.replace("#", ""), 16);
    out[at] = ((v >> 16) & 255) / 255;
    out[at + 1] = ((v >> 8) & 255) / 255;
    out[at + 2] = (v & 255) / 255;
  }

  onMount(() => {
    const cv = canvas;
    if (!cv) return;
    const mode = () => MODES[settings.current.performance_mode] ?? MODES.auto;
    let scale: number = mode().scale;
    const renderSize = () => {
      const w = Math.min(MAX_WIDTH, Math.round(window.innerWidth * scale));
      const h = Math.max(1, Math.round((w * window.innerHeight) / Math.max(1, window.innerWidth)));
      return [w, h] as const;
    };
    // Size the canvas before creating the context.
    [cv.width, cv.height] = renderSize();
    const gl = cv.getContext("webgl", { antialias: false, alpha: false, preserveDrawingBuffer: false });
    if (!gl) {
      console.warn("flow backdrop: WebGL is not available, falling back to the blurred backdrop");
      onUnsupported();
      return;
    }
    const dbg = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer = String(
      dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    );
    report(`flow backdrop: WebGL ${gl.getParameter(gl.VERSION)} on ${renderer}`);
    if (SOFTWARE_RENDERER.test(renderer) && settings.current.performance_mode !== "quality") {
      report("flow backdrop: software rendering, using the blurred backdrop instead");
      onUnsupported();
      return;
    }

    // --- GL resources (rebuilt after a lost context) ---------------------------
    interface Gpu {
      uMix: WebGLUniformLocation | null;
      uHasArt: WebGLUniformLocation | null;
      uTime: WebGLUniformLocation | null;
      uRes: WebGLUniformLocation | null;
      uAmb: WebGLUniformLocation | null;
      uBg: WebGLUniformLocation | null;
      uDim: WebGLUniformLocation | null;
      textures: WebGLTexture[];
    }
    let gpu: Gpu | null = null;

    const compile = (type: number, src: string) => {
      const sh = gl.createShader(type);
      if (!sh) throw new Error("createShader failed");
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) ?? "shader");
      return sh;
    };
    const makeTexture = () => {
      const tex = gl.createTexture();
      if (!tex) throw new Error("createTexture failed");
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RGB,
        1,
        1,
        0,
        gl.RGB,
        gl.UNSIGNED_BYTE,
        new Uint8Array([20, 20, 24]),
      );
      return tex;
    };
    const init = (): boolean => {
      try {
        const prog = gl.createProgram();
        if (!prog) throw new Error("createProgram failed");
        gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
        gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
        gl.linkProgram(prog);
        if (!gl.getProgramParameter(prog, gl.LINK_STATUS))
          throw new Error(gl.getProgramInfoLog(prog) ?? "link");
        gl.useProgram(prog);
        const buf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, buf);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
        const aPos = gl.getAttribLocation(prog, "aPos");
        gl.enableVertexAttribArray(aPos);
        gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
        const u = (name: string) => gl.getUniformLocation(prog, name);
        gl.uniform1i(u("uArtA"), 0);
        gl.uniform1i(u("uArtB"), 1);
        const textures = [makeTexture(), makeTexture()];
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, textures[0]);
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, textures[1]);
        gpu = {
          uMix: u("uMix"),
          uHasArt: u("uHasArt"),
          uTime: u("uTime"),
          uRes: u("uRes"),
          uAmb: u("uAmb"),
          uBg: u("uBg"),
          uDim: u("uDim"),
          textures,
        };
        return true;
      } catch (err) {
        console.warn("flow backdrop unavailable:", String(err));
        gpu = null;
        return false;
      }
    };
    if (!init()) {
      onUnsupported();
      return;
    }

    // --- artwork: two texture slots; a new cover fades in from the idle one ----
    const scratch = document.createElement("canvas");
    scratch.width = scratch.height = TEX_SIZE;
    const ctx2d = scratch.getContext("2d");
    let front = 0; // slot shown when mix settles
    let mix = 0;
    let fadeStart = -1;
    let hasArt = 0;
    let hasArtTarget = 0;
    let loadedSrc: string | null = null;
    let wanted: string | null = null;
    let loadedImg: HTMLImageElement | null = null;

    const upload = (slot: number, img: HTMLImageElement) => {
      if (!gpu || !ctx2d) return;
      ctx2d.drawImage(img, 0, 0, TEX_SIZE, TEX_SIZE);
      gl.activeTexture(slot === 0 ? gl.TEXTURE0 : gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, gpu.textures[slot]);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, scratch);
      gl.generateMipmap(gl.TEXTURE_2D);
    };
    const loadArt = (src: string | null) => {
      wanted = src;
      if (!src) {
        hasArtTarget = 0;
        frames.wake();
        return;
      }
      if (src === loadedSrc) return;
      const img = new Image();
      img.onload = () => {
        if (wanted !== src) return;
        const back = 1 - front;
        upload(back, img);
        loadedSrc = src;
        loadedImg = img;
        front = back;
        fadeStart = performance.now();
        hasArtTarget = 1;
        frames.wake();
        const err = gl.getError();
        if (err !== gl.NO_ERROR) console.warn("flow backdrop: GL error after texture upload", err);
      };
      img.onerror = () => console.warn("flow backdrop: artwork failed to load", src);
      img.src = src;
    };

    // --- colours: targets are converted once per palette change ---------------
    const amb = new Float32Array(15);
    const ambTarget = new Float32Array(15);
    const bg = new Float32Array([0.07, 0.07, 0.09]);
    const bgTarget = new Float32Array(3);
    let dim = 0.35;
    let first = true;
    const easeArray = (cur: Float32Array, target: Float32Array, k: number) => {
      let moving = false;
      for (let i = 0; i < cur.length; i++) {
        const d = target[i] - cur[i];
        if (Math.abs(d) > 0.001) moving = true;
        cur[i] += d * k;
      }
      return moving;
    };

    const resize = () => {
      const [w, h] = renderSize();
      if (cv.width !== w || cv.height !== h) {
        cv.width = w;
        cv.height = h;
        frames.wake();
      }
    };
    window.addEventListener("resize", resize);

    const stopEffects = $effect.root(() => {
      $effect(() => loadArt(palette.textureSrc));
      $effect(() => {
        const p = palette.palette;
        for (let i = 0; i < 5; i++) writeRgb(p.ambient[i] ?? p.dominant, ambTarget, i * 3);
        writeRgb(p.dark_bg, bgTarget, 0);
        frames.wake();
      });
      $effect(() => {
        scale = mode().scale;
        resize();
      });
    });

    // --- lost GPU context (driver reset, suspend/resume) -----------------------
    let lost = false;
    const onLost = (e: Event) => {
      e.preventDefault();
      lost = true;
      gpu = null;
    };
    const onRestored = () => {
      lost = false;
      if (!init()) return;
      // Re-upload the current cover into the front slot.
      if (loadedImg) {
        upload(front, loadedImg);
        mix = front === 1 ? 1 : 0;
      }
      frames.wake();
    };
    cv.addEventListener("webglcontextlost", onLost);
    cv.addEventListener("webglcontextrestored", onRestored);

    // --- frames -----------------------------------------------------------------
    let last = performance.now();
    let lastDraw = -Infinity;
    let time = Math.random() * 60;
    let settling = true;
    // Auto mode: if frames keep arriving late, render fewer pixels.
    let slowFrames = 0;
    const moving = () => player.status === "playing" && !ui.reducedMotion;

    const stopFrames = frames.add({
      needsFrame: () => !lost && (moving() || settling),
      tick(now) {
        if (lost || !gpu) return;
        const dt = Math.min(100, now - last);
        last = now;
        const animate = moving();
        // While only colours settle, draw every frame so the ease is smooth;
        // while moving, cap the rate (the motion itself is slow).
        if (animate && !settling && now - lastDraw < 1000 / mode().fps - 2) return;
        const sinceDraw = Math.min(100, now - lastDraw);
        lastDraw = now;
        if (animate) time += sinceDraw / 1000;
        if (settings.current.performance_mode === "auto" && animate) {
          slowFrames = sinceDraw > 1000 / mode().fps + 20 ? slowFrames + 1 : Math.max(0, slowFrames - 1);
          if (slowFrames > 30 && scale > AUTO_LOW_SCALE) {
            scale = AUTO_LOW_SCALE;
            resize();
            report("flow backdrop: frames run slow, lowering resolution");
          }
        }

        const k = first ? 1 : 1 - Math.exp(-dt / (COLOR_EASE_MS / 3));
        first = false;
        let easing = easeArray(amb, ambTarget, k);
        easing = easeArray(bg, bgTarget, k) || easing;
        const dimTarget = settings.current.backdrop_dim;
        dim += (dimTarget - dim) * k;
        if (Math.abs(dimTarget - dim) > 0.001) easing = true;
        hasArt += (hasArtTarget - hasArt) * (1 - Math.exp(-dt / 400));
        if (Math.abs(hasArtTarget - hasArt) > 0.002) easing = true;
        if (fadeStart >= 0) {
          const f = Math.min(1, (now - fadeStart) / ART_FADE_MS);
          const eased = f * f * (3 - 2 * f);
          // `mix` is the weight of texture slot 1.
          mix = front === 1 ? eased : 1 - eased;
          if (f >= 1) fadeStart = -1;
          else easing = true;
        }
        settling = easing;

        // Use the real drawing-buffer size, which may differ from the canvas size.
        const bw = gl.drawingBufferWidth;
        const bh = gl.drawingBufferHeight;
        gl.viewport(0, 0, bw, bh);
        gl.uniform1f(gpu.uMix, mix);
        gl.uniform1f(gpu.uHasArt, hasArt);
        gl.uniform1f(gpu.uTime, time);
        gl.uniform2f(gpu.uRes, bw, bh);
        gl.uniform3fv(gpu.uAmb, amb);
        gl.uniform3fv(gpu.uBg, bg);
        gl.uniform1f(gpu.uDim, dim);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      },
    });
    // Anything that changes the targets wakes the loop; mark it unsettled.
    const wakeSettle = $effect.root(() => {
      $effect(() => {
        void palette.palette;
        void palette.textureSrc;
        void settings.current.backdrop_dim;
        settling = true;
        frames.wake();
      });
    });

    return () => {
      stopFrames();
      wakeSettle();
      window.removeEventListener("resize", resize);
      cv.removeEventListener("webglcontextlost", onLost);
      cv.removeEventListener("webglcontextrestored", onRestored);
      stopEffects();
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
  });
</script>

<canvas class="flow" bind:this={canvas} aria-hidden="true"></canvas>

<style>
  .flow {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    z-index: -1;
    display: block;
  }
</style>
