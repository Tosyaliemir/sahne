// SPDX-License-Identifier: LicenseRef-MIT-Commons-Clause
// Derived from "Reflective Card" by David Haz, React Bits (https://reactbits.dev),
// MIT License with Commons Clause v1.0. Modified for Sahne. See THIRD_PARTY_NOTICES.md.

// Reflective metal plate: a brushed, beveled card that mirrors a blurred
// image and tilts toward the pointer. It reflects a single <img> only.
// Framework-free apart from the shared frame scheduler; see MetalCover.svelte
// for the wrapper. The tilt and intro are written to CSS variables on `root`:
// --rc-rotate-x/y, --rc-presence, --rc-lift.

import { frames, type FrameClient } from "../frames";

const VIEW_DISTANCE = 900;
const OVERSCAN = 1.2;
const INTRO_MS = 1400;
const FADE_SECONDS = 0.35;

const VERTEX = `#version 300 es
in vec2 aPosition;
out vec2 vUv;
void main() {
  vUv = aPosition * 0.5 + 0.5;
  gl_Position = vec4(aPosition, 0.0, 1.0);
}`;

const CAPTURE = `#version 300 es
precision highp float;
in vec2 vUv;
out vec4 fragColor;
uniform sampler2D uImage;
uniform vec4 uImageMap;
uniform float uWeight;
uniform float uLod;
float level() {
  vec3 mean = pow(textureLod(uImage, vec2(0.5), 14.0).rgb, vec3(2.2));
  return pow(dot(mean, vec3(0.2126, 0.7152, 0.0722)), 1.0 / 2.2);
}
void main() {
  vec3 color = vec3(0.0);
  float mean = 0.0;
  if (uWeight > 0.0) {
    color = textureLod(uImage, vUv * uImageMap.xy + uImageMap.zw, uLod).rgb * uWeight;
    mean = level() * uWeight;
  }
  fragColor = vec4(color, mean);
}`;

const BLUR = `#version 300 es
precision highp float;
in vec2 vUv;
out vec4 fragColor;
uniform sampler2D uTexture;
uniform vec2 uStep;
uniform float uSigma;
void main() {
  vec4 sum = texture(uTexture, vUv);
  if (uSigma < 0.35) {
    fragColor = sum;
    return;
  }
  float falloff = -0.5 / (uSigma * uSigma);
  int radius = int(min(ceil(uSigma * 2.8), 40.0));
  float total = 1.0;
  for (int i = 1; i <= 40; i++) {
    if (i > radius) break;
    float offset = float(i);
    float weight = exp(offset * offset * falloff);
    sum += (texture(uTexture, vUv + uStep * offset) + texture(uTexture, vUv - uStep * offset)) * weight;
    total += 2.0 * weight;
  }
  fragColor = sum / total;
}`;

const PLATE = `#version 300 es
precision highp float;
in vec2 vUv;
out vec4 fragColor;

uniform sampler2D uEnv;
uniform vec2 uEnvSize;
uniform vec2 uSize;
uniform float uRadius;
uniform float uScale;
uniform mat3 uRotate;
uniform float uCapture;
uniform float uBlur;
uniform float uRough;
uniform float uMetal;
uniform vec3 uTint;
uniform float uGray;
uniform float uWarp;
uniform float uWarpScale;
uniform float uShine;
uniform float uBevel;
uniform float uGlare;
uniform vec2 uPointer;
uniform float uTheme;

const float VIEW = ${VIEW_DISTANCE.toFixed(1)};
const float OVERSCAN = ${OVERSCAN.toFixed(2)};
const float CONTENT = 46.0;

float hash(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

vec2 hash2(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.103, 0.0973));
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.xx + p3.yz) * p3.zy) * 2.0 - 1.0;
}

vec3 gradientNoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  vec2 du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
  vec2 ga = hash2(i);
  vec2 gb = hash2(i + vec2(1.0, 0.0));
  vec2 gc = hash2(i + vec2(0.0, 1.0));
  vec2 gd = hash2(i + vec2(1.0, 1.0));
  float va = dot(ga, f);
  float vb = dot(gb, f - vec2(1.0, 0.0));
  float vc = dot(gc, f - vec2(0.0, 1.0));
  float vd = dot(gd, f - vec2(1.0, 1.0));
  float k = va - vb - vc + vd;
  float value = va + u.x * (vb - va) + u.y * (vc - va) + u.x * u.y * k;
  vec2 slope = ga + u.x * (gb - ga) + u.y * (gc - ga) + u.x * u.y * (ga - gb - gc + gd) + du * (u.yx * k + vec2(vb, vc) - va);
  return vec3(value, slope);
}

float valueNoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float erf(float x) {
  float x2 = x * x;
  return sign(x) * sqrt(1.0 - exp(-x2 * (1.2732395 + 0.147 * x2) / (1.0 + 0.147 * x2)));
}

float band(float x, float halfWidth, float sigma) {
  float s = 1.4142136 * max(sigma, 0.0001);
  return 0.5 * (erf((x + halfWidth) / s) - erf((x - halfWidth) / s));
}

float strip(vec2 q, vec2 center, vec2 dir, float halfWidth, vec2 sigma) {
  vec2 normal = vec2(-dir.y, dir.x);
  return band(dot(q - center, normal), halfWidth, length(sigma * normal));
}

const vec2 SWEEP = vec2(0.5, 0.86);
const vec3 LUMA = vec3(0.2126, 0.7152, 0.0722);

float softbox(vec2 q, vec2 sigma) {
  float key = strip(q, vec2(-0.1, -0.16), normalize(SWEEP), 0.022, sigma + 0.018);
  float top = band(q.y + 0.6, 0.12, sigma.y + 0.05);
  float side = band(q.x - 0.46, 0.05, sigma.x + 0.05);
  return key + top * 0.8 + side * 0.35;
}

float room(vec2 q, vec2 sigma) {
  float fall = smoothstep(-0.55, 0.5, q.y);
  float flag = strip(q, vec2(0.24, -0.02), normalize(SWEEP), 0.09, sigma + 0.08);
  return mix(0.85, 0.12, fall) * (1.0 - 0.6 * flag);
}

vec4 sampleEnv(vec2 uv) {
  vec2 st = uv * uEnvSize - 0.5;
  vec2 i = floor(st);
  vec2 f = st - i;
  vec2 f2 = f * f;
  vec2 f3 = f2 * f;
  vec2 w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
  vec2 w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
  vec2 w2 = (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0;
  vec2 w3 = f3 / 6.0;
  vec2 g0 = w0 + w1;
  vec2 g1 = w2 + w3;
  vec2 c0 = (i + 0.5 + w1 / g0 - 1.0) / uEnvSize;
  vec2 c1 = (i + 0.5 + w3 / g1 + 1.0) / uEnvSize;
  vec4 a = texture(uEnv, c0) * g0.x + texture(uEnv, vec2(c1.x, c0.y)) * g1.x;
  vec4 b = texture(uEnv, vec2(c0.x, c1.y)) * g0.x + texture(uEnv, c1) * g1.x;
  return a * g0.y + b * g1.y;
}

vec3 toLinear(vec3 c) {
  return pow(max(c, 0.0), vec3(2.2));
}

void main() {
  vec2 p = vec2(vUv.x - 0.5, 0.5 - vUv.y) * uSize;
  vec2 halfSize = uSize * 0.5;
  float r = min(uRadius, min(halfSize.x, halfSize.y));
  vec2 corner = abs(p) - halfSize + r;
  vec2 side = vec2(p.x >= 0.0 ? 1.0 : -1.0, p.y >= 0.0 ? 1.0 : -1.0);
  float d;
  vec2 outward;
  if (corner.x > 0.0 && corner.y > 0.0) {
    d = length(corner) - r;
    outward = normalize(corner) * side;
  } else if (corner.x > corner.y) {
    d = corner.x - r;
    outward = vec2(side.x, 0.0);
  } else {
    d = corner.y - r;
    outward = vec2(0.0, side.y);
  }
  float coverage = clamp(0.5 - d * uScale, 0.0, 1.0);

  float chamferWidth = min(uBevel, max(r, 1.0));
  float chamfer = chamferWidth > 0.01 ? 1.0 - smoothstep(0.0, chamferWidth, -d) : 0.0;
  vec2 slope = outward * chamfer;

  vec2 wp = p / (220.0 * uWarpScale);
  vec3 w1 = gradientNoise(wp);
  vec3 w2 = gradientNoise(wp * 2.07 + 13.7);
  vec2 ripple = (w1.yz + w2.yz * 0.5) * uWarp;
  slope += ripple * 0.008;

  float grain = valueNoise(vec2(p.x * 0.008, p.y * 0.55)) * 0.45 + valueNoise(vec2(p.x * 0.019 + 3.7, p.y * 0.23)) * 0.35 + valueNoise(vec2(p.x * 0.004 + 9.2, p.y * 0.09)) * 0.2;
  float brush = uRough * uRough;
  slope.y += (grain - 0.5) * brush * 0.004;

  vec3 normal = uRotate * normalize(vec3(slope, 1.0));
  vec3 position = uRotate * vec3(p, 0.0);
  vec3 view = normalize(position - vec3(0.0, 0.0, VIEW));
  vec3 bounce = reflect(view, normal);
  vec2 q = bounce.xy / max(bounce.z, 0.2);

  vec2 footprint = fwidth(q);
  vec2 sigma = sqrt(vec2(uBlur / VIEW) * vec2(uBlur / VIEW) + footprint * footprint * 0.5);
  sigma += vec2(0.004, 0.05) * uRough;

  vec2 shift = clamp((q - p / VIEW) * CONTENT + ripple * 10.0, -uSize, uSize);
  vec2 cp = (p + shift) / (uSize * OVERSCAN);
  vec4 reflected = uCapture > 0.0 ? sampleEnv(vec2(0.5 + cp.x, 0.5 - cp.y)) : vec4(0.0, 0.0, 0.0, 1.0);
  vec3 captured = toLinear(reflected.rgb) * (0.5 / max(pow(reflected.a, 2.2), 0.02));
  captured = mix(vec3(dot(captured, LUMA)), captured, 1.0 - uGray);
  captured = pow(max(captured * 1.2, 0.0), vec3(mix(2.0, 1.5, uTheme)));

  vec3 scene = mix(vec3(room(q, sigma)), captured, uCapture * (1.0 - chamfer) * mix(1.0, 0.6, uTheme));
  scene *= 1.0 + (grain - 0.5) * brush * 0.3;

  vec2 toPointer = (uPointer - p) / (120.0 + uBlur * 4.0);
  vec2 spread = toPointer * vec2(1.0, 1.0 / (1.0 + 2.5 * uRough));
  float reach = dot(spread, spread);
  float glint = max(1.0 + 3.0 * dot(slope, toPointer), 0.0);
  float glare = (exp(-reach * 1.6) + exp(-reach * 0.35) * 0.15) * glint * uGlare * (1.0 + 0.8 * (grain - 0.5) * brush);
  float highlight = (softbox(q, sigma) * uShine + glare) * mix(0.4, 1.0, uMetal);

  vec3 dark = vec3(0.005, 0.005, 0.006) + uTint * (scene * 0.05 * uMetal + highlight * 0.36);
  vec3 light = uTint * (vec3(0.83, 0.83, 0.85) * (1.0 - 0.45 * uMetal * (1.0 - clamp(scene, 0.0, 1.0))) + highlight * 0.35);
  vec3 color = mix(dark, light, uTheme);
  color = min(color, 0.7) + 0.3 * (1.0 - exp(-max(color - 0.7, 0.0) / 0.3));
  color = pow(clamp(color, 0.0, 1.0), vec3(1.0 / 2.2));

  color += (hash(gl_FragCoord.xy) - 0.5) / 255.0;
  color = clamp(color, 0.0, 1.0);
  fragColor = vec4(color * coverage, coverage);
}`;

type Rgb = [number, number, number];

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const parseColor = (value: string | undefined): Rgb => {
  const text = String(value || "").trim();
  const hex = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(text);
  if (hex) {
    const full = hex[1].length === 3 ? hex[1].replace(/./g, (digit) => digit + digit) : hex[1];
    return [0, 2, 4].map((index) => parseInt(full.slice(index, index + 2), 16) / 255) as Rgb;
  }
  const rgb = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/i.exec(text);
  if (rgb) return [rgb[1], rgb[2], rgb[3]].map((channel) => clamp(parseFloat(channel) / 255, 0, 1)) as Rgb;
  return [1, 1, 1];
};

interface Program {
  program: WebGLProgram;
  uniforms: Record<string, WebGLUniformLocation | null>;
}

const compile = (gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null => {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.error(gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
};

const link = (gl: WebGL2RenderingContext, fragmentSource: string): Program | null => {
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, fragmentSource);
  const program = vertex && fragment ? gl.createProgram() : null;
  if (!vertex || !fragment || !program) {
    if (vertex) gl.deleteShader(vertex);
    if (fragment) gl.deleteShader(fragment);
    return null;
  }
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.bindAttribLocation(program, 0, "aPosition");
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error(gl.getProgramInfoLog(program));
    gl.deleteProgram(program);
    return null;
  }
  const uniforms: Program["uniforms"] = {};
  const count: number = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS);
  for (let index = 0; index < count; index++) {
    const info = gl.getActiveUniform(program, index);
    if (info) uniforms[info.name] = gl.getUniformLocation(program, info.name);
  }
  return { program, uniforms };
};

const coverMap = (width: number, height: number, aspect: number): Float32List => {
  const ratio = width / Math.max(1, height);
  const sx = ratio > aspect ? aspect / ratio : 1;
  const sy = ratio > aspect ? 1 : ratio / aspect;
  return [sx, -sy, 0.5 - sx * 0.5, 0.5 + sy * 0.5];
};

const lodFor = (width: number, height: number, envWidth: number, aspect: number) => {
  const ratio = width / Math.max(1, height);
  const visible = width * (ratio > aspect ? aspect / ratio : 1);
  return Math.max(0, Math.log2(Math.max(1, visible / Math.max(1, envWidth))) - 0.5);
};

interface Target {
  texture: WebGLTexture;
  framebuffer: WebGLFramebuffer;
  width: number;
  height: number;
}

/** The image the plate reflects, and how far it has faded in. */
interface Source {
  element: HTMLImageElement;
  src: string;
  ready: boolean;
  failed: boolean;
  width: number;
  height: number;
  mix: number;
}

export interface ReflectiveOptions {
  theme?: "dark" | "light";
  tint?: string;
  metalness?: number;
  roughness?: number;
  blurStrength?: number;
  grayscale?: number;
  warp?: number;
  warpScale?: number;
  shine?: number;
  bevel?: number;
  glare?: number;
  enableTilt?: boolean;
  tiltStrength?: number;
  radius?: number;
  intro?: boolean;
  /** Highest device pixel ratio to render at (1 for battery saver). */
  maxScale?: number;
}

export type ReflectiveSettings = ReturnType<typeof reflectiveSettings>;

const createRenderer = (canvas: HTMLCanvasElement) => {
  const gl = canvas.getContext("webgl2", {
    alpha: true,
    premultipliedAlpha: true,
    antialias: false,
    depth: false,
    stencil: false,
    powerPreference: "default",
  });
  if (!gl) return null;
  const captureProgram = link(gl, CAPTURE);
  const blurProgram = link(gl, BLUR);
  const plateProgram = link(gl, PLATE);
  if (!captureProgram || !blurProgram || !plateProgram) {
    for (const item of [captureProgram, blurProgram, plateProgram]) {
      if (item) gl.deleteProgram(item.program);
    }
    return null;
  }

  const createTexture = (mipmaps: boolean, wrap: number) => {
    const texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, mipmaps ? gl.LINEAR_MIPMAP_LINEAR : gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, wrap);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, wrap);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      1,
      1,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      new Uint8Array([0, 0, 0, 255]),
    );
    return texture;
  };

  const createTarget = (): Target => {
    const texture = createTexture(false, gl.MIRRORED_REPEAT);
    const framebuffer = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return { texture, framebuffer, width: 1, height: 1 };
  };

  const allocate = (target: Target, width: number, height: number) => {
    if (target.width === width && target.height === height) return;
    target.width = width;
    target.height = height;
    gl.bindTexture(gl.TEXTURE_2D, target.texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  };

  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  gl.disable(gl.BLEND);
  gl.disable(gl.DEPTH_TEST);

  const imageTexture = createTexture(true, gl.CLAMP_TO_EDGE);
  const capture = createTarget();
  const blurX = createTarget();
  const blurY = createTarget();
  const rotation = new Float32Array(9);

  const pass = (target: Target | null, program: Program) => {
    gl.bindFramebuffer(gl.FRAMEBUFFER, target ? target.framebuffer : null);
    gl.viewport(0, 0, target ? target.width : canvas.width, target ? target.height : canvas.height);
    gl.useProgram(program.program);
    return program.uniforms;
  };

  const bind = (unit: number, texture: WebGLTexture) => {
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, texture);
  };

  const upload = (element: HTMLImageElement) => {
    try {
      gl.bindTexture(gl.TEXTURE_2D, imageTexture);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, element);
      gl.generateMipmap(gl.TEXTURE_2D);
      return true;
    } catch {
      return false;
    }
  };

  const renderEnv = ({
    width,
    height,
    scale,
    blur,
    roughness,
    image,
  }: {
    width: number;
    height: number;
    scale: number;
    blur: number;
    roughness: number;
    image: Source;
  }) => {
    const envScale = clamp(2.5 / Math.max(blur, 0.01), 0.12, scale);
    const envWidth = Math.max(8, Math.ceil(width * OVERSCAN * envScale));
    const envHeight = Math.max(8, Math.ceil(height * OVERSCAN * envScale));
    allocate(capture, envWidth, envHeight);
    allocate(blurX, envWidth, envHeight);
    allocate(blurY, envWidth, envHeight);
    const aspect = width / Math.max(1, height);

    let u = pass(capture, captureProgram);
    bind(0, imageTexture);
    gl.uniform1i(u.uImage, 0);
    gl.uniform4fv(u.uImageMap, image.ready ? coverMap(image.width, image.height, aspect) : [1, 1, 0, 0]);
    gl.uniform1f(u.uWeight, image.ready && image.mix > 0 ? 1 : 0);
    gl.uniform1f(u.uLod, image.ready ? lodFor(image.width, image.height, envWidth, aspect) : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    u = pass(blurX, blurProgram);
    bind(0, capture.texture);
    gl.uniform1i(u.uTexture, 0);
    gl.uniform2f(u.uStep, 1 / envWidth, 0);
    gl.uniform1f(u.uSigma, blur * envScale);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    u = pass(blurY, blurProgram);
    bind(0, blurX.texture);
    gl.uniform1i(u.uTexture, 0);
    gl.uniform2f(u.uStep, 0, 1 / envHeight);
    gl.uniform1f(u.uSigma, (blur * (1 + 1.5 * roughness) + 10 * roughness) * envScale);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  const renderPlate = ({
    width,
    height,
    rotateX,
    rotateY,
    capture: amount,
    glare,
    pointerX,
    pointerY,
    settings,
  }: {
    width: number;
    height: number;
    rotateX: number;
    rotateY: number;
    capture: number;
    glare: number;
    pointerX: number;
    pointerY: number;
    settings: ReflectiveSettings;
  }) => {
    const cx = Math.cos(rotateX);
    const sx = Math.sin(rotateX);
    const cy = Math.cos(rotateY);
    const sy = Math.sin(rotateY);
    rotation.set([cy, sx * sy, -cx * sy, 0, cx, sx, sy, -sx * cy, cx * cy]);

    const u = pass(null, plateProgram);
    bind(0, blurY.texture);
    gl.uniform1i(u.uEnv, 0);
    gl.uniform2f(u.uEnvSize, blurY.width, blurY.height);
    gl.uniform2f(u.uSize, width, height);
    gl.uniform1f(u.uRadius, settings.radius);
    gl.uniform1f(u.uScale, canvas.width / Math.max(1, width));
    gl.uniformMatrix3fv(u.uRotate, false, rotation);
    gl.uniform1f(u.uCapture, amount);
    gl.uniform1f(u.uBlur, settings.blurStrength);
    gl.uniform1f(u.uRough, settings.roughness);
    gl.uniform1f(u.uMetal, settings.metalness);
    gl.uniform3fv(u.uTint, settings.tint);
    gl.uniform1f(u.uGray, settings.grayscale);
    gl.uniform1f(u.uWarp, settings.warp);
    gl.uniform1f(u.uWarpScale, settings.warpScale);
    gl.uniform1f(u.uShine, settings.shine);
    gl.uniform1f(u.uBevel, settings.bevel);
    gl.uniform1f(u.uGlare, glare);
    gl.uniform2f(u.uPointer, pointerX, pointerY);
    gl.uniform1f(u.uTheme, settings.theme);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  const dispose = () => {
    if (gl.isContextLost()) return;
    for (const target of [capture, blurX, blurY]) {
      gl.deleteFramebuffer(target.framebuffer);
      gl.deleteTexture(target.texture);
    }
    gl.deleteTexture(imageTexture);
    gl.deleteBuffer(buffer);
    gl.deleteVertexArray(vao);
    for (const item of [captureProgram, blurProgram, plateProgram]) gl.deleteProgram(item.program);
  };

  return { upload, renderEnv, renderPlate, dispose };
};

/** Normalizes user-facing options into what the renderer reads each frame. */
export function reflectiveSettings({
  theme = "dark",
  tint = "#ffffff",
  metalness = 1,
  roughness = 0.3,
  blurStrength = 6,
  grayscale = 1,
  warp = 0.6,
  warpScale = 1,
  shine = 1,
  bevel = 3,
  glare = 0.5,
  enableTilt = true,
  tiltStrength = 12,
  radius = 24,
  intro = true,
  maxScale = 2,
}: ReflectiveOptions = {}) {
  const palette = theme === "light" ? "light" : "dark";
  return {
    tint: parseColor(tint).map((channel) => Math.pow(channel, 2.2)) as Rgb,
    metalness: clamp(metalness, 0, 1),
    roughness: clamp(roughness, 0, 1),
    blurStrength: clamp(blurStrength, 0, 40),
    grayscale: clamp(grayscale, 0, 1),
    warp: clamp(warp, 0, 2),
    warpScale: clamp(warpScale, 0.2, 4),
    shine: clamp(shine, 0, 3),
    bevel: clamp(bevel, 0, 16),
    glare: clamp(glare, 0, 1),
    enableTilt,
    tiltStrength: clamp(tiltStrength, 0, 30),
    radius: Math.max(0, radius),
    theme: palette === "light" ? 1 : 0,
    intro,
    maxScale: clamp(maxScale, 1, 2),
  };
}

export interface ReflectivePlate {
  /** Draws at least one more frame (e.g. the options or the image changed). */
  wake(): void;
  destroy(): void;
}

/**
 * `root` receives the pointer and the CSS variables, `body` is measured,
 * `canvas` draws the plate, `image` is the <img> it reflects. Frames come
 * from the shared scheduler, so nothing is drawn while the window is
 * hidden.
 */
export function createReflectivePlate(
  {
    root,
    body,
    canvas,
    image,
  }: { root: HTMLElement; body: HTMLElement; canvas: HTMLCanvasElement; image: HTMLImageElement },
  getSettings: () => ReflectiveSettings,
  { isFirst = true, onRestore = () => {} }: { isFirst?: boolean; onRestore?: () => void } = {},
): ReflectivePlate {
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const renderer = createRenderer(canvas);
  const x = { p: 0, v: 0, target: 0 };
  const y = { p: 0, v: 0, target: 0 };
  const light = { p: 0, target: 0 };
  const pointer = { x: 0, y: 0 };
  const size = { width: body.offsetWidth, height: body.offsetHeight, scale: 1 };
  const source: Source = {
    element: image,
    src: "",
    ready: false,
    failed: false,
    width: 0,
    height: 0,
    mix: 0,
  };
  let presence = 1;
  let presenceStart = 0;
  let slowUntil = 0;
  let touchId: number | null = null;
  let last = performance.now();
  /** Whether the last frame asked for another one. */
  let busy = true;
  let visible = true;
  let lost = false;
  let envKey = "";
  let envFresh = false;

  const write = () => {
    const { enableTilt: tiltOn, tiltStrength: strength } = getSettings();
    const angle = tiltOn && !reduced ? strength : 0;
    const style = root.style;
    style.setProperty("--rc-rotate-x", `${(y.p * angle).toFixed(3)}deg`);
    style.setProperty("--rc-rotate-y", `${(-x.p * angle).toFixed(3)}deg`);
    const eased = 1 - Math.pow(1 - presence, 3);
    style.setProperty("--rc-presence", eased.toFixed(4));
    style.setProperty("--rc-lift", `${((1 - eased) * 14).toFixed(2)}px`);
  };

  /** Uploads the image once per new src; true when the texture changed. */
  const refresh = () => {
    const element = source.element;
    const src = element.currentSrc;
    if (src !== source.src) {
      source.src = src;
      source.failed = false;
      source.ready = false;
    }
    if (!src || source.ready || source.failed) return false;
    if (!element.complete || !element.naturalWidth) return false;
    if (!renderer || !renderer.upload(element)) {
      source.failed = true;
      return false;
    }
    source.width = element.naturalWidth;
    source.height = element.naturalHeight;
    source.ready = true;
    return true;
  };

  const fade = (dt: number) => {
    const target = source.ready && !source.failed && source.src ? 1 : 0;
    const before = source.mix;
    source.mix += (target - source.mix) * (1 - Math.exp(-dt / FADE_SECONDS));
    if (Math.abs(target - source.mix) < 0.002) source.mix = target;
    return source.mix !== before;
  };

  const resize = () => {
    size.width = body.offsetWidth;
    size.height = body.offsetHeight;
    size.scale = Math.min(window.devicePixelRatio || 1, getSettings().maxScale);
    const w = Math.max(1, Math.round(size.width * size.scale));
    const h = Math.max(1, Math.round(size.height * size.scale));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    envFresh = true;
  };

  const frame = (now: number) => {
    if (lost) return;
    // The resolution cap is a setting (battery saver); follow it.
    if (Math.min(window.devicePixelRatio || 1, getSettings().maxScale) !== size.scale) resize();
    const dt = Math.min(0.05, (now - last) / 1000 || 0);
    last = now;
    const slow = now < slowUntil;
    const stiffness = slow ? 24 : 120;
    const damping = slow ? 9 : 16;
    const steps = Math.max(1, Math.ceil(dt / (1 / 120)));
    const h = dt / steps;
    for (let i = 0; i < steps; i++) {
      for (const axis of [x, y]) {
        axis.v += (stiffness * (axis.target - axis.p) - damping * axis.v) * h;
        axis.p += axis.v * h;
      }
    }
    const tau = light.target > light.p ? 0.18 : slow ? 0.9 : 0.45;
    light.p += (light.target - light.p) * (1 - Math.exp(-dt / tau));
    if (presence < 1) presence = clamp((now - presenceStart) / 650, 0, 1);

    const moving =
      Math.abs(x.target - x.p) > 0.0005 ||
      Math.abs(y.target - y.p) > 0.0005 ||
      Math.abs(x.v) > 0.0005 ||
      Math.abs(y.v) > 0.0005 ||
      Math.abs(light.target - light.p) > 0.002 ||
      presence < 1;
    if (!moving) {
      x.p = x.target;
      y.p = y.target;
      x.v = 0;
      y.v = 0;
      light.p = light.target;
    }
    write();

    let fading = false;
    if (renderer) {
      if (refresh()) envFresh = true;
      if (fade(dt)) envFresh = true;
      if (source.mix > 0 && source.mix < 1) fading = true;
      const s = getSettings();
      const key = `${s.blurStrength}|${s.roughness}|${size.width}|${size.height}|${size.scale}`;
      if (key !== envKey) {
        envKey = key;
        envFresh = true;
      }
      if (envFresh && source.mix > 0) {
        renderer.renderEnv({
          width: size.width,
          height: size.height,
          scale: size.scale,
          blur: s.blurStrength,
          roughness: s.roughness,
          image: source,
        });
      }
      envFresh = false;
      const angle = (s.enableTilt && !reduced ? s.tiltStrength : 0) * (Math.PI / 180);
      renderer.renderPlate({
        width: size.width,
        height: size.height,
        rotateX: y.p * angle,
        rotateY: -x.p * angle,
        capture: source.mix,
        glare: s.glare * light.p,
        pointerX: pointer.x,
        pointerY: pointer.y,
        settings: s,
      });
    }

    busy = moving || fading;
  };

  const client: FrameClient = {
    tick: frame,
    needsFrame: () => busy && visible && !lost,
  };

  const wake = () => {
    if (lost || !visible) return;
    if (!busy) last = performance.now();
    frames.request(client);
  };

  const aim = (nx: number, ny: number, on = true) => {
    x.target = clamp(nx, -1, 1);
    y.target = clamp(ny, -1, 1);
    light.target = on ? 1 : 0;
    slowUntil = 0;
    wake();
  };

  const aimAt = (event: PointerEvent) => {
    const rect = root.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const nx = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    const ny = ((event.clientY - rect.top) / rect.height) * 2 - 1;
    pointer.x = nx * size.width * 0.5;
    pointer.y = ny * size.height * 0.5;
    aim(nx, ny);
  };

  const release = () => aim(0, 0, false);

  const onEnter = (event: PointerEvent) => {
    if (event.pointerType === "mouse") aimAt(event);
  };
  const onMove = (event: PointerEvent) => {
    if (event.pointerType === "mouse" || event.pointerId === touchId) aimAt(event);
  };
  const onLeave = (event: PointerEvent) => {
    if (event.pointerType === "mouse") release();
  };
  const onDown = (event: PointerEvent) => {
    if (event.pointerType === "mouse") return;
    touchId = event.pointerId;
    aimAt(event);
  };
  const onUp = (event: PointerEvent) => {
    if (event.pointerId !== touchId) return;
    touchId = null;
    release();
  };

  root.addEventListener("pointerenter", onEnter);
  root.addEventListener("pointermove", onMove);
  root.addEventListener("pointerleave", onLeave);
  root.addEventListener("pointerdown", onDown);
  root.addEventListener("pointerup", onUp);
  root.addEventListener("pointercancel", onUp);

  const onLost = (event: Event) => {
    event.preventDefault();
    lost = true;
  };
  const onRestored = () => onRestore();
  canvas.addEventListener("webglcontextlost", onLost);
  canvas.addEventListener("webglcontextrestored", onRestored);

  const resizeObserver = new ResizeObserver(() => {
    resize();
    wake();
  });
  resizeObserver.observe(body);
  resize();

  const intersection = new IntersectionObserver((entries) => {
    visible = entries.some((entry) => entry.isIntersecting);
    wake();
  });
  intersection.observe(root);

  const onImage = () => wake();
  image.addEventListener("load", onImage);
  image.addEventListener("error", onImage);

  if (getSettings().intro && !reduced && isFirst) {
    x.p = 0.62;
    y.p = -0.55;
    light.p = 1;
    pointer.x = 0.62 * size.width * 0.5;
    pointer.y = -0.55 * size.height * 0.5;
    presence = 0;
    presenceStart = performance.now();
    slowUntil = performance.now() + INTRO_MS;
  }
  write();
  const stopFrames = frames.add(client);

  const destroy = () => {
    stopFrames();
    resizeObserver.disconnect();
    intersection.disconnect();
    root.removeEventListener("pointerenter", onEnter);
    root.removeEventListener("pointermove", onMove);
    root.removeEventListener("pointerleave", onLeave);
    root.removeEventListener("pointerdown", onDown);
    root.removeEventListener("pointerup", onUp);
    root.removeEventListener("pointercancel", onUp);
    canvas.removeEventListener("webglcontextlost", onLost);
    canvas.removeEventListener("webglcontextrestored", onRestored);
    image.removeEventListener("load", onImage);
    image.removeEventListener("error", onImage);
    renderer?.dispose();
  };

  return { wake, destroy };
}
