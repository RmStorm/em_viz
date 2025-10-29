import createREGL from "regl";

export function startRenderer(canvas: HTMLCanvasElement) {
	const gl2 = canvas.getContext("webgl2", {
		antialias: true,
		alpha: false,
		premultipliedAlpha: false,
		preserveDrawingBuffer: false,
	}) as WebGL2RenderingContext | null;

	if (!gl2) {
		throw new Error(
			"This app requires WebGL2. Your browser/GPU does not provide a WebGL2 context.",
		);
	}
	const regl = createREGL({ gl: gl2 });

	const resize = () => {
		const dpr = Math.min(window.devicePixelRatio || 1, 2);
		const w = Math.floor(canvas.clientWidth * dpr);
		const h = Math.floor(canvas.clientHeight * dpr);
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
			regl._gl.viewport(0, 0, w, h);
		}
	};
	resize();
	window.addEventListener("resize", resize);

	let staging: Uint8Array | null = null;

	const tex = regl.texture({
		width: 2,
		height: 2,
		data: new Uint8Array(4),
		format: "luminance",
		type: "uint8",
	});

	function updateFieldFromExEy(
		width: number,
		height: number,
		exey: Float32Array,
	) {
		const N = width * height;
		if (!staging || staging.length !== N) staging = new Uint8Array(N);
		for (let i = 0, j = 0; i < N; i++, j += 2) {
			let m = Math.hypot(exey[j], exey[j + 1]);
			m = m / (1 + m);
			staging[i] = (m * 255) | 0;
		}
		tex({
			width,
			height,
			data: staging,
			format: "luminance",
			type: "uint8",
		});
	}

	const draw = regl({
		vert: `#version 300 es
precision mediump float;
in vec2 pos;
out vec2 uv;
void main(){
  uv = 0.5 * (pos + 1.0);
  gl_Position = vec4(pos, 0.0, 1.0);
}`,
		frag: `#version 300 es
precision mediump float;
in vec2 uv;
uniform sampler2D fieldTex;
out vec4 fragColor;

vec3 viridis(float t){
  const vec3 c0 = vec3(0.267, 0.005, 0.329);
  const vec3 c1 = vec3(0.283, 0.141, 0.458);
  const vec3 c2 = vec3(0.254, 0.266, 0.530);
  const vec3 c3 = vec3(0.207, 0.372, 0.553);
  const vec3 c4 = vec3(0.164, 0.471, 0.558);
  const vec3 c5 = vec3(0.993, 0.906, 0.144);
  float x = clamp(t, 0.0, 1.0) * 5.0;
  float i = floor(x);
  float f = x - i;
  if (i < 1.0) return mix(c0, c1, f);
  else if (i < 2.0) return mix(c1, c2, f);
  else if (i < 3.0) return mix(c2, c3, f);
  else if (i < 4.0) return mix(c3, c4, f);
  else return mix(c4, c5, f);
}

void main(){
  float g = texture(fieldTex, uv).r;
  float t = pow(g, 0.75);
  fragColor = vec4(viridis(t), 1.0);
}`,
		attributes: { pos: [-1, -1, 1, -1, -1, 1, 1, 1] },
		count: 4,
		primitive: "triangle strip",
		uniforms: { fieldTex: tex },
	});

	const drawLineStrip = (() => {
		const posBuffer = regl.buffer({
			usage: "dynamic",
			type: "float",
			length: 0,
		});

		const draw = regl({
			vert: `
precision mediump float;
attribute vec2 pos;   // clip-space
void main(){ gl_Position = vec4(pos, 0.0, 1.0); }`,
			frag: `
precision mediump float;
uniform vec4 uColor;
void main(){ gl_FragColor = uColor; }`,
			attributes: { pos: posBuffer },
			primitive: "line strip",
			count: regl.prop<"count", "count">("count"),
			uniforms: { uColor: regl.prop<"uColor", "uColor">("uColor") },
			blend: {
				enable: true,
				func: {
					srcRGB: "src alpha",
					srcAlpha: "one",
					dstRGB: "one minus src alpha",
					dstAlpha: "one minus src alpha",
				},
			},
			depth: { enable: false },
		});

		return (
			clipPoints: Float32Array,
			color: [number, number, number, number],
		) => {
			posBuffer(clipPoints);
			draw({ count: clipPoints.length / 2, uColor: color });
		};
	})();

	function drawLines(
		all: Float32Array[],
		color: [number, number, number, number] = [1, 1, 1, 0.9],
	) {
		for (const L of all) drawLineStrip(L, color);
	}
	return { draw, drawLines, updateFieldFromExEy, regl };
}
