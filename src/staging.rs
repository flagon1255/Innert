//! Host memory that what's built goes over to the GPU through, a piece at a time. It's made once
//! and kept: making hundreds of megabytes of it for each new piece of the world stalled the GPU
//! while Windows got it ready, and the frames being drawn stalled with it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage, PrimaryAutoCommandBuffer,
        PrimaryCommandBufferAbstract, allocator::StandardCommandBufferAllocator,
    },
    device::Queue,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    sync::{
        GpuFuture,
        future::{FenceSignalFuture, NowFuture},
    },
};

/// Words in each piece, 4 MB. Each takes a millisecond or so to go over, so a frame being drawn
/// meanwhile never waits long behind one; with pieces of 32 MB they held frames up by several.
pub const PIECE_WORDS: usize = 1 << 20;

/// The most pieces on their way at once, see pieces_at_once(), 64 MB of host memory in all
const SLOTS: usize = 16;

/// About how long the GPU takes to copy one piece over, in milliseconds
const PIECE_MS: f64 = 0.35;

/// The pieces of host memory, used by turns
static PIECES: OnceLock<Mutex<Vec<Subbuffer<[u32]>>>> = OnceLock::new();

/// How long the frames being drawn take, in tenths of a millisecond, see frame_time()
static FRAME_TIME: AtomicU32 = AtomicU32::new(0);

/// Says how long the frames being drawn take, so what's sent meanwhile goes over as quickly as it
/// can without holding them up much, see pieces_at_once()
pub fn frame_time(ms: f64) {
    FRAME_TIME.store((ms * 10.0).clamp(0.0, u32::MAX as f64) as u32, Ordering::Relaxed);
}

/// How many pieces may be on their way at once. Each lands behind the frames already sent, and
/// holds up the one after it as it's copied, so frames that take long can have more of them
/// between them: about a tenth of a frame's time goes to copying, at least two pieces' worth. With
/// only two at a time, sending the world over while frames took 130 ms took half a minute.
fn pieces_at_once() -> usize {
    let frame_ms = FRAME_TIME.load(Ordering::Relaxed) as f64 / 10.0;
    ((frame_ms * 0.1 / PIECE_MS) as usize).clamp(2, SLOTS)
}

/// The most words of `whole`-word rows or slices that fit in a piece
pub fn whole(rows: usize) -> usize {
    (PIECE_WORDS / rows.max(1)).max(1) * rows.max(1)
}

/// Sends `len` words over to the GPU, `piece` at most at a time (whole rows or slices of an image,
/// say, see whole()): `fill` writes the words from the given one on into the piece it's handed,
/// then `copy` records copying that piece to wherever it goes. Each piece is waited for on a fence
/// before its host memory's used again, so frames drawn meanwhile get the GPU in between, never
/// held up behind the lot.
pub fn send(
    memory_allocator: &Arc<StandardMemoryAllocator>,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    queue: &Arc<Queue>,
    len: usize,
    piece: usize,
    mut fill: impl FnMut(&mut [u32], usize),
    mut copy: impl FnMut(&mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, Subbuffer<[u32]>, usize),
) {
    let pieces = PIECES.get_or_init(|| {
        let make = || {
            Buffer::new_slice::<u32>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                PIECE_WORDS as u64,
            )
            .unwrap()
        };
        Mutex::new((0..SLOTS).map(|_| make()).collect())
    });
    // One lot at a time, should a piece of the world and something else be sent at once
    let pieces = pieces.lock().unwrap_or_else(|e| e.into_inner());
    let piece = piece.clamp(1, PIECE_WORDS);
    // Which pieces of host memory are free, and those on their way, oldest first
    let mut free: Vec<usize> = (0..pieces.len()).rev().collect();
    let mut going: VecDeque<(usize, FenceSignalFuture<CommandBufferExecFuture<NowFuture>>)> = VecDeque::new();
    let mut first = 0;
    while first < len {
        while going.len() >= pieces_at_once() || free.is_empty() {
            let (slot, gone) = going.pop_front().unwrap();
            gone.wait(None).unwrap();
            free.push(slot);
        }
        let slot = free.pop().unwrap();
        let n = piece.min(len - first);
        let staged = pieces[slot].clone().slice(0..n as u64);
        fill(&mut staged.write().unwrap(), first);
        let mut builder = AutoCommandBufferBuilder::primary(command_buffer_allocator.clone(), queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
        copy(&mut builder, staged, first);
        going.push_back((slot, builder.build().unwrap().execute(queue.clone()).unwrap().then_signal_fence_and_flush().unwrap()));
        first += n;
    }
    for (_, gone) in going {
        gone.wait(None).unwrap();
    }
}
