import init, { Solver } from "../crates/solver/pkg/solver.js";

let solver: Solver;
let view: Float32Array; // [Ex, Ey, Ex, Ey, ...]
let W = 0,
	H = 0;

export async function initSolver(width: number, height: number) {
	const wasmExports = await init();
	if (wasmExports === null) {
		throw "Burp";
	}
	solver = new Solver(width, height);
	W = width;
	H = height;

	const len = W * H * 2;
	const ptr = solver.field_ptr();
	const memory = wasmExports.memory as WebAssembly.Memory;

	view = new Float32Array(memory.buffer, ptr, len);
	return { solver, view };
}

export function updateSolver(time: number) {
	solver.step(time);
	return view;
}

// Helper: read one cell (for tooltip)
export function sampleCell(ix: number, iy: number) {
	const base = (iy * W + ix) * 2;
	const Ex = view[base] ?? 0;
	const Ey = view[base + 1] ?? 0;
	const mag = Math.hypot(Ex, Ey);
	return { Ex, Ey, mag };
}
