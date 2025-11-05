# em_viz

Web-based experiments for visualising electromagnetic fields. The original prototype was WebGL; this branch is the in-progress WebGPU port built with Leptos and Tailwind.

## Prerequisites

- Rust toolchain with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).
- [`trunk`](https://trunkrs.dev) for building and serving (`cargo install trunk`).
- Tailwind is driven through Trunk (see `Trunk.toml`); no extra npm tooling is required.

## Running

- `trunk serve --open` — hot-reloads the Leptos client app.
- `trunk build --release` — produces the optimised `dist/` output.

## Controls & Debugging

- Toggle Electric (E) ribbons, tweak the per-charge seed count, and slide the charge impostor point size directly in the sidebar.
- The debug panel mirrors camera matrices, eye position, total ribbons dispatched, last GPU dispatch time, and reports validation or compute errors pulled from the renderer.
- Use the “Rebuild now” button after dragging charges if you want to re-trigger the GPU compute without changing other sliders.
- The sidebar debug panel now breaks down seed preparation, GPU dispatch, and readback times so you can spot bottlenecks in the pipeline.

## Licence

Distributed under the GNU GPL v3 (`LICENSE`). Derivative work must remain GPL-compatible, and redistributions must provide source code under the same terms.
