use crate::em3d::Charge3D;
use crate::perf_gpu::GpuTimerRing;
use leptos::{logging::*, prelude::*};
use web_sys::HtmlCanvasElement;
use wgpu::{self, util::DeviceExt};

// ------- Fixed pool sizes (tweak if needed) -------
const MAX_STREAMS: u32 = 1024; // ribbons per dispatch
const MAX_PTS: u32 = 1600; // RK steps per ribbon
const MAX_CHARGES: u32 = 64; // max charges

// Derived sizes
const COUNT_BYTES: u64 = (MAX_STREAMS as u64) * 16; // draw indirect args per stream
const SEEDS_BYTES: u64 = (MAX_STREAMS as u64) * 16; // vec4 per seed
const CHARGES_BYTES: u64 = (MAX_CHARGES as u64) * 16; // vec4 per charge
// Each RK step emits two vertices; each vertex is 2 * vec4<f32> (packed like in WGSL)
// => 2 verts * 2 vec4 * 16B = 64B per step
const OUT_BYTES: u64 = (MAX_STREAMS as u64) * (MAX_PTS as u64) * 64;

#[derive(Clone, Copy, Debug)]
pub struct DispatchStats {
    pub streams: u32,
    pub gpu_ms: f64,
    pub zero_ms: f64,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RibbonV {
    // matches [px,py,pz, tx,ty,tz, side, tone]
    data: [f32; 8],
}
impl RibbonV {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        let stride = mem::size_of::<RibbonV>() as wgpu::BufferAddress; // 32
        wgpu::VertexBufferLayout {
            array_stride: stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    shader_location: 0,
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x3,
                }, // pos
                wgpu::VertexAttribute {
                    shader_location: 1,
                    offset: 12,
                    format: wgpu::VertexFormat::Float32x3,
                }, // tangent
                wgpu::VertexAttribute {
                    shader_location: 2,
                    offset: 24,
                    format: wgpu::VertexFormat::Float32,
                }, // side
                wgpu::VertexAttribute {
                    shader_location: 3,
                    offset: 28,
                    format: wgpu::VertexFormat::Float32,
                }, // tone
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    center: [f32; 3],
    _pad: f32, // 16B alignment
}
impl Instance {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[wgpu::VertexAttribute {
                shader_location: 2,
                offset: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadV {
    pos: [f32; 2], // -0.5..+0.5 screen-aligned quad
}
impl QuadV {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<QuadV>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                shader_location: 0,
                offset: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

const QUAD: [QuadV; 6] = [
    QuadV { pos: [-0.5, -0.5] },
    QuadV { pos: [0.5, -0.5] },
    QuadV { pos: [0.5, 0.5] },
    QuadV { pos: [-0.5, -0.5] },
    QuadV { pos: [0.5, 0.5] },
    QuadV { pos: [-0.5, 0.5] },
];

const SPHERE_SHADER: &str = include_str!("../static/shaders/sphere.wgsl");
const RIBBON_SHADER: &str = include_str!("../static/shaders/ribbon.wgsl");
const RIBBON_COMP: &str = include_str!("../static/shaders/ribbon_e_comp.wgsl");

#[derive(Debug)]
pub struct Charges {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    ubo: wgpu::Buffer, // view, proj, viewport.x/y, point_size
    vbuf_quad: wgpu::Buffer,
    ibuf_instances: wgpu::Buffer,
    instances_len: u32,
    ibuf_size_bytes: u64,
}

impl Charges {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let smod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("charges shader"),
            source: wgpu::ShaderSource::Wgsl(SPHERE_SHADER.into()),
        });

        // bind layout (uniform only)
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("charges u_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // 2*mat4 + vec4(viewport.x, viewport.y, point_size, 0) = 144 bytes
        let ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("charges ubo"),
            size: 144,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("charges u_bg"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ubo.as_entire_binding(),
            }],
        });

        // pipeline
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("charges pl"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("charges pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &smod,
                entry_point: Some("vs"),
                buffers: &[QuadV::layout(), Instance::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &smod,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // geometry buffers
        let vbuf_quad = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("charges quad vbuf"),
            contents: bytemuck::cast_slice(&QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let ibuf_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("charges instances"),
            size: 16, // start tiny, grow as needed
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_layout,
            bind_group,
            ubo,
            vbuf_quad,
            ibuf_instances,
            instances_len: 0,
            ibuf_size_bytes: 16,
        }
    }

    pub fn update_viewproj(
        &self,
        queue: &wgpu::Queue,
        viewport: [f32; 2],
        point_size_px: f32,
        view: [f32; 16],
        proj: [f32; 16],
    ) {
        let mut bytes = [0u8; 144];
        bytes[0..64].copy_from_slice(bytemuck::cast_slice(&view));
        bytes[64..128].copy_from_slice(bytemuck::cast_slice(&proj));
        let v = [viewport[0], viewport[1], point_size_px, 0.0];
        bytes[128..144].copy_from_slice(bytemuck::cast_slice(&v));
        queue.write_buffer(&self.ubo, 0, &bytes);
    }

    pub fn update_charges(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        charges: &[Charge3D],
    ) {
        // NOTE: this Instance is the vertex-instancing struct you already defined above.
        let instances: Vec<Instance> = charges
            .iter()
            .map(|c| Instance {
                center: [c.pos.x, c.pos.y, c.pos.z],
                _pad: 0.0,
            })
            .collect();

        let bytes = bytemuck::cast_slice(&instances);
        let needed = bytes.len() as u64;

        if needed > self.ibuf_size_bytes {
            self.ibuf_instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("charges.instances"),
                size: needed,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.ibuf_size_bytes = needed;
        }

        queue.write_buffer(&self.ibuf_instances, 0, bytes);
        self.instances_len = instances.len() as u32;
    }

    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if self.instances_len == 0 {
            return;
        }
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vbuf_quad.slice(..));
        rpass.set_vertex_buffer(1, self.ibuf_instances.slice(..));
        rpass.draw(0..6, 0..self.instances_len);
    }
}

pub struct ERibbonsCompute {
    pipeline: wgpu::ComputePipeline,
    bind_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    ubo: wgpu::Buffer, // k, soft2, h, max_pts, far_cut
    buf_charges: wgpu::Buffer,
    buf_seeds: wgpu::Buffer,
    pub buf_counts: wgpu::Buffer,
}

impl ERibbonsCompute {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, ribbon_vbuf_e: &wgpu::Buffer) -> Self {
        let comp_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ribbon_e_comp"),
            source: wgpu::ShaderSource::Wgsl(RIBBON_COMP.into()),
        });

        // bind layout: ubo + charges + seeds + out verts + counts
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("comp E layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // UBO
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // charges (RO)
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // seeds (RO)
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // OUT_VERTS (RW)
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // COUNTS (RW) + INDIRECT
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("comp E pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("comp E pl"),
                    bind_group_layouts: &[&bind_layout],
                    push_constant_ranges: &[],
                }),
            ),
            module: &comp_mod,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // buffers
        let ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comp ubo"),
            size: 48, // 3 * vec4
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let buf_charges = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("charges"),
            size: CHARGES_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let buf_seeds = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("seeds"),
            size: SEEDS_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let buf_counts = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("counts"),
            size: COUNT_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("comp E bg"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubo.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf_charges.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buf_seeds.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ribbon_vbuf_e.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: buf_counts.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            bind_layout,
            bind_group,
            ubo,
            buf_charges,
            buf_seeds,
            buf_counts,
        }
    }

    pub fn upload_inputs(&mut self, queue: &wgpu::Queue, charges: &[[f32; 4]], seeds: &[[f32; 4]]) {
        queue.write_buffer(&self.buf_charges, 0, bytemuck::cast_slice(charges));
        queue.write_buffer(&self.buf_seeds, 0, bytemuck::cast_slice(seeds));
    }

    pub fn write_params(
        &self,
        queue: &wgpu::Queue,
        k: f32,
        soft2: f32,
        h: f32,
        max_pts: u32,
        far_cut: f32,
    ) {
        queue.write_buffer(
            &self.ubo,
            0,
            bytemuck::cast_slice(&[
                k,
                soft2,
                h,
                max_pts as f32,
                far_cut,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ]),
        );
    }
}

#[derive(Debug)]
pub struct ERibbonsDraw {
    ribbon_pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    ubo: wgpu::Buffer,  // view, proj, viewport.x/y, halfWidth, alpha
    vbuf: wgpu::Buffer, // OUT vertices written by compute
    streams_active: u32,
}

impl ERibbonsDraw {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, vbuf: wgpu::Buffer) -> Self {
        let ribbon_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ribbon shader"),
            source: wgpu::ShaderSource::Wgsl(RIBBON_SHADER.into()),
        });

        // UBO
        let ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ribbon ubo"),
            size: 144, // view, proj, vec4(viewport.x, viewport.y, halfWidth, alpha)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // bind layout/group
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ribbon ubo layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ribbon ubo bg"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ubo.as_entire_binding(),
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ribbon pl"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let ribbon_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ribbon pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &ribbon_mod,
                entry_point: Some("vs"),
                buffers: &[RibbonV::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ribbon_mod,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            ribbon_pipeline,
            bind_layout,
            bind_group,
            ubo,
            vbuf,
            streams_active: 0,
        }
    }

    pub fn draw<'a>(
        &'a self,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'a>,
        counts: &'a wgpu::Buffer, // from compute
        viewport: [f32; 2],       // from renderer
        view: [f32; 16],          // from renderer
        proj: [f32; 16],          // from renderer
    ) {
        if self.streams_active == 0 {
            return;
        }

        // write UBO (view, proj, viewport.x/y, halfWidth, alpha)
        let mut bytes = [0u8; 144];
        bytes[0..64].copy_from_slice(bytemuck::cast_slice(&view));
        bytes[64..128].copy_from_slice(bytemuck::cast_slice(&proj));
        // tweak thickness/alpha here:
        let v = [viewport[0], viewport[1], 2.0, 0.85];
        bytes[128..144].copy_from_slice(bytemuck::cast_slice(&v));
        queue.write_buffer(&self.ubo, 0, &bytes);

        // draw
        rpass.set_vertex_buffer(0, self.vbuf.slice(..));
        rpass.set_pipeline(&self.ribbon_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        for i in 0..self.streams_active {
            let offset = (i as u64) * 16;
            rpass.draw_indirect(counts, offset);
        }
    }

    pub fn set_streams(&mut self, n: u32) {
        self.streams_active = n;
    }
}

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    viewport: [f32; 2],
    point_size_px: f32,
    last_view: [f32; 16],
    last_proj: [f32; 16],

    charges: Charges,
    ecomp: ERibbonsCompute,
    edraw: ERibbonsDraw,

    timer: GpuTimerRing,
}

impl WgpuRenderer {
    pub async fn new(
        canvas: HtmlCanvasElement,
        initial_charges: &[Charge3D],
        point_px: f32,
    ) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                ..Default::default()
            })
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let size = (canvas.width(), canvas.height());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        // shared ribbon vertex buffer (compute writes / draw reads)
        let ribbon_vbuf_e = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ribbon_vbuf_e"),
            size: OUT_BYTES,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // sub-systems
        let charges = Charges::new(&device, format);
        let ecomp = ERibbonsCompute::new(&device, &queue, &ribbon_vbuf_e);
        let edraw = ERibbonsDraw::new(&device, format, ribbon_vbuf_e.clone());

        let timer = GpuTimerRing::new(&device, &queue, "Ecomp");

        let mut this = Self {
            surface,
            device,
            queue,
            config,
            size,
            viewport: [size.0 as f32, size.1 as f32],
            point_size_px: point_px.max(1.0),
            last_view: [0.0; 16],
            last_proj: [0.0; 16],
            charges,
            ecomp,
            edraw,
            timer,
        };

        // initial charges upload (once)
        this.charges
            .update_charges(&this.device, &this.queue, initial_charges);

        Ok(this)
    }

    pub fn update_viewproj(&mut self, view: [f32; 16], proj: [f32; 16]) {
        self.last_view = view;
        self.last_proj = proj;
        self.charges
            .update_viewproj(&self.queue, self.viewport, self.point_size_px, view, proj);
    }

    pub fn set_point_size(&mut self, px: f32) {
        self.point_size_px = px.max(1.0);
    }

    pub fn update_charges(&mut self, charges: &[Charge3D]) {
        self.charges
            .update_charges(&self.device, &self.queue, charges);
    }
    pub fn start_compute_ribbons_e(
        &mut self,
        charges: &[[f32; 4]],
        seeds: &[[f32; 4]],
        h: f32,
        max_pts: u32,
    ) {
        let k = 1.0f32;
        let soft2 = 0.0025f32;
        let far_cut = 250.0f32;
        self.ecomp
            .write_params(&self.queue, k, soft2, h, max_pts, far_cut);
        self.ecomp.upload_inputs(&self.queue, charges, seeds);
        // self.queue.submit([]);
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Ribbon compute encoder"),
            });
        let (ts_writes, finalize) = self.timer.span_compute("Ribbon compute encoder");
        {
            let mut c = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Ribbon compute pass descriptor"),
                timestamp_writes: Some(ts_writes),
            });
            c.set_pipeline(&self.ecomp.pipeline);
            c.set_bind_group(0, &self.ecomp.bind_group, &[]);
            let groups = (seeds.len() as u32).div_ceil(64);
            c.dispatch_workgroups(groups, 1, 1);
        } // compute pass dropped to drop the encoders borrow!
        finalize(&self.queue, enc);
        self.edraw.set_streams(seeds.len() as u32);
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                self.surface.get_current_texture()?
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });
        let (ts_writes, finalize) = self.timer.span_render("render");
        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rpass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.05,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: Some(ts_writes),
                occlusion_query_set: None,
            });

            // ribbons (single pass)
            self.edraw.draw(
                &self.queue,
                &mut rpass,
                &self.ecomp.buf_counts,
                self.viewport,
                self.last_view,
                self.last_proj,
            );

            // spheres
            self.charges.draw(&mut rpass);
        }
        finalize(&self.queue, enc);
        frame.present();
        Ok(())
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 || (w, h) == self.size {
            return;
        }
        self.size = (w, h);
        self.config.width = w;
        self.config.height = h;
        self.viewport = [w as f32, h as f32];
        self.surface.configure(&self.device, &self.config);
    }
    pub fn clear_ribbons(&mut self) {
        self.edraw.set_streams(0);
    }
}
