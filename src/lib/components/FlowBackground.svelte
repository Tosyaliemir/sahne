<script lang="ts">
  import { onMount } from "svelte";
  import { palette } from "../stores/palette.svelte";
  import { ui } from "../stores/ui.svelte";
  import { settings } from "../stores/settings.svelte";

  /**
   * "Akışkan" backdrop: the album art itself, in two layers turning in
   * opposite directions and bent by a slowly moving noise field, blended
   * with the ambient palette and finished with fine grain against banding.
   * Rendered at reduced resolution and scaled up (it is soft by design).
   * Calls `onUnsupported` if WebGL is unavailable so the caller can fall back.
   */
  let { onUnsupported }: { onUnsupported: () => void } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();

  const RENDER_SCALE = 0.4;
  const MAX_WIDTH = 960;
  const TEX_SIZE = 256;
  const ART_FADE_MS = 1400;
  const COLOR_EASE_MS = 900;

  const VERT = `
    attribute vec2 aPos;
    varying vec2 vUv;
    void main() {
      vUv = aPos * 0.5 + 0.5;
      gl_Position = vec4(aPos, 0.0, 1.0);
    }`;

  const FRAG = `
    precision mediump float;
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
      vec2 a = rot(t * 0.9) * p * 0.85 + (r - 0.5) * 0.6;
      vec2 b = rot(-t * 0.6 + 1.3) * p * 1.35 + (q - 0.5) * 0.5 + vec2(0.31, -0.17);
      a.x /= aspect;
      b.x /= aspect;
      vec3 cover = mix(art(a + 0.5), art(b + 0.5), smoothstep(0.3, 0.7, r.x));

      vec3 amb = ambient(fbm(p * 0.9 + r * 1.4 + t * 0.4) * 1.6 + t * 0.15);
      vec3 col = mix(amb, cover, 0.62 * uHasArt);

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

  function hexToRgb(hex: string): [number, number, number] {
    const v = parseInt(hex.replace("#", ""), 16);
    return [((v >> 16) & 255) / 255, ((v >> 8) & 255) / 255, (v & 255) / 255];
  }

  onMount(() => {
    const gl = canvas?.getContext("webgl", { antialias: false, alpha: false, preserveDrawingBuffer: false });
    if (!canvas || !gl) {
      onUnsupported();
      return;
    }

    const compile = (type: number, src: string) => {
      const sh = gl.createShader(type)!;
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) ?? "shader");
      return sh;
    };
    let prog: WebGLProgram;
    try {
      prog = gl.createProgram()!;
      gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
      gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) ?? "link");
    } catch (err) {
      console.warn("flow backdrop unavailable:", String(err));
      onUnsupported();
      return;
    }
    gl.useProgram(prog);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(prog, "aPos");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    const u = (name: string) => gl.getUniformLocation(prog, name);
    const uMix = u("uMix");
    const uHasArt = u("uHasArt");
    const uTime = u("uTime");
    const uRes = u("uRes");
    const uAmb = u("uAmb");
    const uBg = u("uBg");
    const uDim = u("uDim");
    gl.uniform1i(u("uArtA"), 0);
    gl.uniform1i(u("uArtB"), 1);

    // Two texture slots; a new cover goes into the idle one and fades in.
    const scratch = document.createElement("canvas");
    scratch.width = scratch.height = TEX_SIZE;
    const ctx2d = scratch.getContext("2d")!;
    const makeTexture = () => {
      const tex = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, 1, 1, 0, gl.RGB, gl.UNSIGNED_BYTE, new Uint8Array([20, 20, 24]));
      return tex;
    };
    const textures = [makeTexture(), makeTexture()];
    let front = 0; // slot shown when mix settles
    let mix = 0;
    let fadeStart = -1;
    let hasArt = 0;
    let hasArtTarget = 0;

    const upload = (slot: number, img: HTMLImageElement) => {
      ctx2d.drawImage(img, 0, 0, TEX_SIZE, TEX_SIZE);
      gl.activeTexture(slot === 0 ? gl.TEXTURE0 : gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, textures[slot]);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, scratch);
      gl.generateMipmap(gl.TEXTURE_2D);
    };
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, textures[0]);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, textures[1]);

    let loadedSrc: string | null = null;
    let wanted: string | null = null;
    const loadArt = (src: string | null) => {
      wanted = src;
      if (!src) {
        hasArtTarget = 0;
        return;
      }
      if (src === loadedSrc) return;
      const img = new Image();
      img.onload = () => {
        if (wanted !== src) return;
        const back = 1 - front;
        upload(back, img);
        loadedSrc = src;
        // Fade from the current front to the new back slot.
        front = back;
        fadeStart = performance.now();
        hasArtTarget = 1;
      };
      img.onerror = () => console.warn("flow backdrop: artwork failed to load");
      img.src = src;
    };

    // Colors ease toward their targets instead of jumping.
    let amb = new Float32Array(15);
    let bg = [0.07, 0.07, 0.09];
    let dim = 0.35;
    let first = true;

    const resize = () => {
      const w = Math.min(MAX_WIDTH, Math.round(window.innerWidth * RENDER_SCALE));
      const h = Math.max(1, Math.round((w * window.innerHeight) / Math.max(1, window.innerWidth)));
      if (canvas!.width !== w || canvas!.height !== h) {
        canvas!.width = w;
        canvas!.height = h;
        gl.viewport(0, 0, w, h);
      }
    };
    resize();
    window.addEventListener("resize", resize);

    const stopArt = $effect.root(() => {
      $effect(() => loadArt(palette.textureSrc));
    });

    let last = performance.now();
    let time = Math.random() * 1000;
    let raf = requestAnimationFrame(function frame(now) {
      const dt = Math.min(100, now - last);
      last = now;
      if (!ui.reducedMotion) time += dt / 1000;

      const p = palette.palette;
      const k = first ? 1 : 1 - Math.exp(-dt / (COLOR_EASE_MS / 3));
      first = false;
      p.ambient.slice(0, 5).forEach((hex, i) => {
        const [r, g, b] = hexToRgb(hex);
        amb[i * 3] += (r - amb[i * 3]) * k;
        amb[i * 3 + 1] += (g - amb[i * 3 + 1]) * k;
        amb[i * 3 + 2] += (b - amb[i * 3 + 2]) * k;
      });
      const [br, bgG, bb] = hexToRgb(p.dark_bg);
      bg = [bg[0] + (br - bg[0]) * k, bg[1] + (bgG - bg[1]) * k, bg[2] + (bb - bg[2]) * k];
      dim += (settings.current.backdrop_dim - dim) * k;
      hasArt += (hasArtTarget - hasArt) * (1 - Math.exp(-dt / 400));

      if (fadeStart >= 0) {
        const f = Math.min(1, (now - fadeStart) / ART_FADE_MS);
        const eased = f * f * (3 - 2 * f);
        // `mix` is the weight of texture slot 1.
        mix = front === 1 ? eased : 1 - eased;
        if (f >= 1) fadeStart = -1;
      }

      gl.uniform1f(uMix, mix);
      gl.uniform1f(uHasArt, hasArt);
      gl.uniform1f(uTime, time);
      gl.uniform2f(uRes, canvas!.width, canvas!.height);
      gl.uniform3fv(uAmb, amb);
      gl.uniform3f(uBg, bg[0], bg[1], bg[2]);
      gl.uniform1f(uDim, dim);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      raf = requestAnimationFrame(frame);
    });

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      stopArt();
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
