//! Host memory that what's built goes over to the GPU through, a piece at a time. It's made once
//! and kept: making hundreds of megabytes of it for each new piece of the world stalled the GPU
//! while Windows got it ready, and the frames being drawn stalled with it.

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

/// Two pieces, so one can be filled while the other goes over
static PIECES: OnceLock<Mutex<[Subbuffer<[u32]>; 2]>> = OnceLock::new();

/// The most words of `whole`-word rows or slices that fit in a piece
pub fn whole(rows: usize) -> usize {
    (PIECE_WORDS / rows.max(1)).max(1) * rows.max(1)
}

/// Sends `len` words over to the GPU, `piece` at most at a time (whole rows or slices of an image,
/// say, see whole()): `fill` writes the words from the given one on into the piece it's handed,
/// then `copy` records copying that piece to wherever it goes. Each piece is waited for on a fence,
/// so frames drawn meanwhile get the GPU in between, never held up behind the lot.
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
        Mutex::new([make(), make()])
    });
    // One lot at a time, should a piece of the world and something else be sent at once
    let pieces = pieces.lock().unwrap_or_else(|e| e.into_inner());
    let piece = piece.clamp(1, PIECE_WORDS);
    let mut going: [Option<FenceSignalFuture<CommandBufferExecFuture<NowFuture>>>; 2] = [None, None];
    let (mut first, mut k) = (0, 0);
    while first < len {
        let n = piece.min(len - first);
        let slot = k % 2;
        if let Some(gone) = going[slot].take() {
            gone.wait(None).unwrap();
        }
        let staged = pieces[slot].clone().slice(0..n as u64);
        fill(&mut staged.write().unwrap(), first);
        let mut builder = AutoCommandBufferBuilder::primary(command_buffer_allocator.clone(), queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
        copy(&mut builder, staged, first);
        going[slot] = Some(builder.build().unwrap().execute(queue.clone()).unwrap().then_signal_fence_and_flush().unwrap());
        first += n;
        k += 1;
    }
    for gone in going.into_iter().flatten() {
        gone.wait(None).unwrap();
    }
}
