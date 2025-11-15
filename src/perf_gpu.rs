use leptos::logging::log;
use wgpu::{ComputePassTimestampWrites, MapMode, RenderPassTimestampWrites};

const NUM_SLOTS: usize = 32;
const SLOT_BYTES: u64 = 16; // 2 * u64 timestamps

struct Slot {
    resolve_buf: wgpu::Buffer,
    read_buf: wgpu::Buffer,
}

pub struct GpuTimerRing {
    qset: wgpu::QuerySet,
    slots: [Slot; NUM_SLOTS],
    head: usize,
    period_ns: f64, // on webgpu always 1 ns.. on other platforms timer ticks can be different
}

impl GpuTimerRing {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, label: &'static str) -> Self {
        let qset = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some(&format!("{label}.qset")),
            ty: wgpu::QueryType::Timestamp,
            count: (NUM_SLOTS * 2) as u32,
        });

        let slots: [Slot; NUM_SLOTS] = std::array::from_fn(|i| {
            let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label}.resolve[{i}]")),
                size: SLOT_BYTES,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label}.read[{i}]")),
                size: SLOT_BYTES,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            Slot {
                resolve_buf,
                read_buf,
            }
        });

        Self {
            qset,
            slots,
            head: 0,
            period_ns: queue.get_timestamp_period() as f64,
        }
    }
    fn span_inner<'a>(
        &'a self,
        idx: usize,
        label: &'static str,
    ) -> (
        u32, // begin_q
        u32, // end_q
        impl FnOnce(&wgpu::Queue, wgpu::CommandEncoder) + 'a,
    ) {
        let qset = &self.qset;
        let begin_q = (idx * 2) as u32;
        let end_q = begin_q + 1;

        let period_ns = self.period_ns;
        let slot = &self.slots[idx];

        let finalize = move |queue: &wgpu::Queue, mut encoder: wgpu::CommandEncoder| {
            // 1) Resolve the query set and copy the result to a mappable buffer
            encoder.resolve_query_set(qset, begin_q..(end_q + 1), &slot.resolve_buf, 0);
            encoder.copy_buffer_to_buffer(&slot.resolve_buf, 0, &slot.read_buf, 0, SLOT_BYTES);

            // 2) Schedule the encoder for execution
            queue.submit(Some(encoder.finish()));

            // 3) Schedule the map_async callback. guaranteed to run _after_ encoder completion!
            let read_buf = slot.read_buf.clone();
            slot.read_buf
                .slice(..)
                .map_async(MapMode::Read, move |res| {
                    if let Err(e) = res {
                        log!("Mapping {} slot {} failed: {:?}", label, idx, e);
                        return;
                    }

                    let dt_ms = {
                        let data = read_buf.slice(..).get_mapped_range();
                        let t0 = u64::from_le_bytes(data[0..8].try_into().unwrap());
                        let t1 = u64::from_le_bytes(data[8..16].try_into().unwrap());

                        let dt_ticks = t1.saturating_sub(t0) as f64;
                        let dt_ns = dt_ticks * period_ns;
                        dt_ns / 1_000_000.0
                    };
                    read_buf.unmap();
                    log!("[gpu] {}: {:.3} ms", label, dt_ms);
                });
        };

        (begin_q, end_q, finalize)
    }

    pub fn span_compute<'a>(
        &'a mut self,
        label: &'static str,
    ) -> (
        ComputePassTimestampWrites<'a>,
        impl FnOnce(&wgpu::Queue, wgpu::CommandEncoder) + 'a,
    ) {
        // Notice that this is kinda racy... we rely on slots to be empty before we modulo around to them again!
        let idx = self.head;
        self.head = (idx + 1) % NUM_SLOTS;

        let (begin_q, end_q, finalize) = self.span_inner(idx, label);

        let ts_writes = ComputePassTimestampWrites {
            query_set: &self.qset,
            beginning_of_pass_write_index: Some(begin_q),
            end_of_pass_write_index: Some(end_q),
        };

        (ts_writes, finalize)
    }

    pub fn span_render<'a>(
        &'a mut self,
        label: &'static str,
    ) -> (
        RenderPassTimestampWrites<'a>,
        impl FnOnce(&wgpu::Queue, wgpu::CommandEncoder) + 'a,
    ) {
        // Notice that this is kinda racy... we rely on slots to be empty before we modulo around to them again!
        let idx = self.head;
        self.head = (idx + 1) % NUM_SLOTS;

        let (begin_q, end_q, finalize) = self.span_inner(idx, label);

        let ts_writes = RenderPassTimestampWrites {
            query_set: &self.qset,
            beginning_of_pass_write_index: Some(begin_q),
            end_of_pass_write_index: Some(end_q),
        };

        (ts_writes, finalize)
    }
}
