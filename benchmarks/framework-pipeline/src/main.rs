use std::hint::black_box;
use std::time::Instant;

const NODE_COUNT: usize = 4_096;
const ROUNDS: usize = 7;
const LAYOUT_DIRTY: u8 = 1;
const PAINT_DIRTY: u8 = 2;

#[derive(Clone, Copy)]
struct Node {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    value: u32,
    opacity: u32,
    dirty: u8,
}

#[derive(Clone, Copy)]
struct DrawCommand {
    id: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: u32,
}

#[derive(Default)]
struct Damage {
    initialized: bool,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl Damage {
    fn include(&mut self, node: &Node) {
        let right = node.x.wrapping_add(node.width);
        let bottom = node.y.wrapping_add(node.height);
        if !self.initialized {
            self.initialized = true;
            self.left = node.x;
            self.top = node.y;
            self.right = right;
            self.bottom = bottom;
            return;
        }
        self.left = self.left.min(node.x);
        self.top = self.top.min(node.y);
        self.right = self.right.max(right);
        self.bottom = self.bottom.max(bottom);
    }

    fn checksum(&self) -> u64 {
        u64::from(self.left)
            ^ (u64::from(self.top) << 11)
            ^ (u64::from(self.right) << 22)
            ^ (u64::from(self.bottom) << 33)
    }
}

struct EventModule;

impl EventModule {
    fn dispatch(&self, nodes: &mut [Node], random: &mut u64, count: usize) {
        for event_index in 0..count {
            let target = next_index(random, nodes.len());
            let node = &mut nodes[target];
            node.value = node
                .value
                .wrapping_add(((*random >> 24) as u32 & 31).wrapping_add(1));
            node.dirty |= PAINT_DIRTY;
            if event_index & 7 == 0 {
                node.dirty |= LAYOUT_DIRTY;
            }
        }
    }
}

struct AnimationModule;

impl AnimationModule {
    fn advance(&self, nodes: &mut [Node], random: &mut u64, count: usize) {
        for change_index in 0..count {
            let target = next_index(random, nodes.len());
            let node = &mut nodes[target];
            node.opacity = node.opacity.wrapping_add(17) & 255;
            node.dirty |= PAINT_DIRTY;
            if change_index & 15 == 0 {
                node.value = node.value.wrapping_add(3);
                node.dirty |= LAYOUT_DIRTY;
            }
        }
    }
}

struct LayoutModule;

impl LayoutModule {
    fn update(&self, nodes: &mut [Node]) {
        for index in 0..nodes.len() {
            if nodes[index].dirty & LAYOUT_DIRTY == 0 {
                continue;
            }
            let (parent_x, parent_y) = if index == 0 {
                (0, 0)
            } else {
                let parent = nodes[(index - 1) / 4];
                (parent.x, parent.y)
            };
            let slot = (index & 3) as u32;
            let value = nodes[index].value;
            nodes[index].x = parent_x.wrapping_add(slot * 7 + value % 5);
            nodes[index].y = parent_y.wrapping_add(slot * 5 + value % 7);
            nodes[index].width = 24 + value % 97;
            nodes[index].height = 16 + (value.wrapping_mul(3)) % 53;
            nodes[index].dirty |= PAINT_DIRTY;
        }
    }
}

struct PaintModule {
    commands: Vec<DrawCommand>,
}

impl PaintModule {
    fn new() -> Self {
        Self {
            commands: Vec::with_capacity(NODE_COUNT),
        }
    }

    fn record(&mut self, nodes: &mut [Node]) -> Damage {
        self.commands.clear();
        let mut damage = Damage::default();
        for (index, node) in nodes.iter_mut().enumerate() {
            if node.dirty & PAINT_DIRTY == 0 {
                continue;
            }
            damage.include(node);
            self.commands.push(DrawCommand {
                id: index as u32,
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
                color: node
                    .value
                    .wrapping_mul(2_654_435_761)
                    .rotate_left(node.opacity & 31),
            });
            node.dirty = 0;
        }
        damage
    }

    fn submit_checksum(&self) -> u64 {
        self.commands.iter().fold(0_u64, |checksum, command| {
            checksum
                .rotate_left(7)
                .wrapping_add(u64::from(command.id) * 3)
                .wrapping_add(u64::from(command.x) * 5)
                .wrapping_add(u64::from(command.y) * 7)
                .wrapping_add(u64::from(command.width) * 11)
                .wrapping_add(u64::from(command.height) * 13)
                .wrapping_add(u64::from(command.color))
        })
    }
}

struct Framework {
    nodes: Vec<Node>,
    events: EventModule,
    animation: AnimationModule,
    layout: LayoutModule,
    paint: PaintModule,
    random: u64,
    checksum: u64,
}

impl Framework {
    fn new() -> Self {
        let nodes = (0..NODE_COUNT)
            .map(|index| Node {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                value: (index as u32).wrapping_mul(17).wrapping_add(11),
                opacity: (index as u32).wrapping_mul(13) & 255,
                dirty: LAYOUT_DIRTY | PAINT_DIRTY,
            })
            .collect();
        Self {
            nodes,
            events: EventModule,
            animation: AnimationModule,
            layout: LayoutModule,
            paint: PaintModule::new(),
            random: 0x4d59_5df4_d0f3_3173,
            checksum: 0,
        }
    }

    fn frame(&mut self, frame_index: usize, events: usize, changes: usize) {
        self.events
            .dispatch(&mut self.nodes, &mut self.random, events);
        self.animation
            .advance(&mut self.nodes, &mut self.random, changes);
        self.layout.update(&mut self.nodes);
        let damage = self.paint.record(&mut self.nodes);
        self.checksum = self
            .checksum
            .rotate_left(9)
            .wrapping_add(self.paint.submit_checksum())
            ^ damage.checksum();
        if frame_index.is_multiple_of(60) {
            self.checksum ^= semantic_checksum(&self.nodes);
        }
    }
}

fn next_index(random: &mut u64, length: usize) -> usize {
    *random = random
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*random >> 32) as usize) % length
}

fn semantic_checksum(nodes: &[Node]) -> u64 {
    nodes
        .iter()
        .step_by(4)
        .enumerate()
        .fold(0_u64, |checksum, (index, node)| {
            checksum
                .rotate_left(3)
                .wrapping_add(index as u64)
                .wrapping_add(u64::from(node.value))
                .wrapping_add(u64::from(node.opacity) << 17)
        })
}

fn run(frames: usize, events: usize, changes: usize) -> (u128, u64) {
    let mut framework = Framework::new();
    let started = Instant::now();
    for frame_index in 0..frames {
        framework.frame(frame_index, events, changes);
    }
    (started.elapsed().as_nanos(), black_box(framework.checksum))
}

fn benchmark(name: &str, frames: usize, events: usize, changes: usize) {
    let _ = run(100, events, changes);
    let mut samples = Vec::with_capacity(ROUNDS);
    let mut checksum = 0;
    for _ in 0..ROUNDS {
        let (elapsed, result) = run(frames, events, changes);
        samples.push(elapsed);
        checksum ^= result;
    }
    samples.sort_unstable();
    let median = samples[ROUNDS / 2];
    let frame_ns = median as f64 / frames as f64;
    println!(
        "workload={name} frames={frames} median_ns={median} ns_per_frame={frame_ns:.1} frames_per_second={:.1} checksum={checksum}",
        1_000_000_000.0 / frame_ns
    );
}

fn main() {
    benchmark("steady", 4_000, 4, 32);
    benchmark("interactive", 2_000, 32, 512);
}
