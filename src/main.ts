import Alpine from "alpinejs";
import { initMathJax } from "./mathjax-init";
import { startRenderer } from "./render";
import { initSolver, sampleCell, updateSolver } from "./solver";

(window as any).Alpine = Alpine;
Alpine.start();
Alpine.store("app", {
	fieldLines: { visible: false, seedsPerAxis: 16 },
	tooltip: {
		visible: false,
		ix: 99,
		iy: 112,
		u: 0.39010416666666664,
		v: 0.4391304347826087,
		x: 1183,
		y: 317,
		Ex: 0,
		Ey: 0,
		mag: 0,
	},
});
initMathJax();

const W = 256,
	H = 256;

// bilinear sample Ex,Ey at normalized (u,v)
function fieldAt(
	u: number,
	v: number,
	W: number,
	H: number,
	view: Float32Array,
) {
	// clamp to domain (avoid a stray NaN if RK4 steps out)
	u = Math.max(0, Math.min(1, u));
	v = Math.max(0, Math.min(1, v));

	const x = u * (W - 1),
		y = v * (H - 1);
	const x0 = Math.floor(x),
		y0 = Math.floor(y);
	const x1 = Math.min(W - 1, x0 + 1),
		y1 = Math.min(H - 1, y0 + 1);
	const tx = x - x0,
		ty = y - y0;

	const i00 = (y0 * W + x0) * 2,
		i10 = (y0 * W + x1) * 2;
	const i01 = (y1 * W + x0) * 2,
		i11 = (y1 * W + x1) * 2;

	const Ex00 = view[i00],
		Ey00 = view[i00 + 1];
	const Ex10 = view[i10],
		Ey10 = view[i10 + 1];
	const Ex01 = view[i01],
		Ey01 = view[i01 + 1];
	const Ex11 = view[i11],
		Ey11 = view[i11 + 1];

	const Ex0 = Ex00 * (1 - tx) + Ex10 * tx,
		Ex1 = Ex01 * (1 - tx) + Ex11 * tx;
	const Ey0 = Ey00 * (1 - tx) + Ey10 * tx,
		Ey1 = Ey01 * (1 - tx) + Ey11 * tx;

	return { Ex: Ex0 * (1 - ty) + Ex1 * ty, Ey: Ey0 * (1 - ty) + Ey1 * ty };
}

function normDir(Ex: number, Ey: number) {
	const m = Math.hypot(Ex, Ey);
	if (m < 1e-6) return { dx: 0, dy: 0, mag: m };
	return { dx: Ex / m, dy: Ey / m, mag: m };
}

// one RK4 step along +/- field direction; h is in UV units
function rk4(
	u: number,
	v: number,
	h: number,
	sign: number,
	view: Float32Array,
) {
	const f = (uu: number, vv: number) => {
		const { Ex, Ey } = fieldAt(uu, vv, W, H, view);
		const d = normDir(Ex, Ey);
		return { dx: d.dx * sign, dy: d.dy * sign, mag: d.mag };
	};

	const k1 = f(u, v);
	const k2 = f(u + 0.5 * h * k1.dx, v + 0.5 * h * k1.dy);
	const k3 = f(u + 0.5 * h * k2.dx, v + 0.5 * h * k2.dy);
	const k4 = f(u + h * k3.dx, v + h * k3.dy);

	const dx = (k1.dx + 2 * k2.dx + 2 * k3.dx + k4.dx) / 6.0;
	const dy = (k1.dy + 2 * k2.dy + 2 * k3.dy + k4.dy) / 6.0;

	return { u: u + h * dx, v: v + h * dy, mag: k1.mag };
}

// integrate from a seed (u,v) → clip-space polyline
function integrateStreamline(
	seedU: number,
	seedV: number,
	stepScale = 0.7,
	maxPts = 500,
	view: Float32Array,
) {
	const h = stepScale / Math.max(W, H); // UV step relative to grid size

	function walk(sign: number) {
		const pts: number[] = [];
		let u = seedU,
			v = seedV;
		for (let i = 0; i < maxPts; i++) {
			if (u <= 0 || u >= 1 || v <= 0 || v >= 1) break;
			// push clip-space position (y flip so top-left is (0,0))
			pts.push(u * 2 - 1, 1 - v * 2);

			const s = rk4(u, v, h, sign, view);
			u = s.u;
			v = s.v;

			if (s.mag < 1e-5) break; // stagnation
			if (s.mag > 1e3) break; // near charge core
		}
		return pts;
	}

	const neg = walk(-1);
	const fwd = walk(+1);

	// merge: reverse neg (omit seed dup) + fwd
	const merged = new Float32Array(
		(neg.length > 0 ? neg.length - 2 : 0) + fwd.length,
	);
	let o = 0;
	for (let i = neg.length - 2; i >= 2; i -= 2) {
		merged[o++] = neg[i - 2];
		merged[o++] = neg[i - 1];
	}
	for (let i = 0; i < fwd.length; i++) {
		merged[o++] = fwd[i];
	}
	return merged;
}

// jittered grid seeds
function makeSeeds(n: number) {
	const out: { u: number; v: number }[] = [];
	for (let j = 0; j < n; j++)
		for (let i = 0; i < n; i++) {
			const u = (i + 0.5 + (Math.random() - 0.5) * 0.3) / n;
			const v = (j + 0.5 + (Math.random() - 0.5) * 0.3) / n;
			out.push({ u, v });
		}
	return out;
}

(async () => {
	const canvas = document.getElementById("gl") as HTMLCanvasElement;
	const { draw, updateFieldFromExEy, drawLines } = startRenderer(canvas);

	const store = (window as any).Alpine.store("app");

	const { solver, view } = await initSolver(W, H);

	solver.clear_charges();
	solver.add_charge(0.35, 0.5, +1.0);
	solver.add_charge(0.65, 0.5, -1.0);
	let lines: Float32Array[] = [];
	let linesDirty = true;

	function recomputeLines() {
		const n = store.fieldLines.seedsPerAxis ?? 16;
		const seeds = makeSeeds(n);
		lines = [];
		for (const s of seeds) {
			lines.push(integrateStreamline(s.u, s.v, 0.8, 600, view));
		}
		linesDirty = false;
	}

	const orig = store.fieldLines.seedsPerAxis;
	Object.defineProperty(store.fieldLines, "seedsPerAxis", {
		get() {
			return this._n ?? orig ?? 16;
		},
		set(v: number) {
			this._n = v;
			linesDirty = true;
		},
	});

	function onMove(e: MouseEvent) {
		if (!store.tooltip.visible) return;
		const rect = canvas.getBoundingClientRect();
		const u = (e.clientX - rect.left) / rect.width;
		const v = (e.clientY - rect.top) / rect.height;

		const ix = Math.max(0, Math.min(W - 1, Math.floor(u * W)));
		const iy = Math.max(0, Math.min(H - 1, Math.floor(v * H)));
		const { Ex, Ey, mag } = sampleCell(ix, iy);
		store.tooltip = {
			visible: true,
			x: e.clientX + 14,
			y: e.clientY + 14,
			ix,
			iy,
			u,
			v,
			Ex,
			Ey,
			mag,
		};
	}
	canvas.addEventListener("mousemove", onMove);

	function frame(time: number) {
		console.log(1, linesDirty,store.fieldLines.visible)
		updateSolver(time * 0.001);
		updateFieldFromExEy(W, H, view);
		if (store.fieldLines.visible && linesDirty) {
		console.log(2)
			recomputeLines();
		}
		draw();
		if (store.fieldLines.visible) {
		console.log(3)
			drawLines(lines); // white 1px lines
		}
		requestAnimationFrame(frame);
	}
	requestAnimationFrame(frame);
})();
