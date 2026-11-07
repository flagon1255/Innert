//! What the game needs of the graphics card that isn't the same on every one, see
//! vulkan.gpuinfo.org. NVIDIA's are generous. AMD's and Intel's aren't, nor are the Vulkan spec's
//! minimums: an RX 470 under Windows has room for 128 bytes of push constants where NVIDIA's have
//! 256, and AMD's and Intel's have 2048 texels along a 3D image where NVIDIA's have 16384. Intel's
//! integrated graphics bind no more than 1 GB of a storage buffer, and 16 storage images to a
//! shader.
//!
//! Each shader's checked against the card's as its pipeline's made, see Limits::shortfalls(), so a
//! card short of something says so plainly rather than the game falling over, and a test checks
//! every shader against MIN. What's sized to the card is sized to fit, see memory.rs.
//!
//! `--set limits=min` takes the card to have no more than MIN, nor VK_EXT_memory_budget, nor any of
//! its memory the CPU can write to directly, as on an AMD card without Resizable BAR once its 256 MB
//! window is used up: to try out on an NVIDIA card what the game does on those. `--set gpu=intel`
//! picks the graphics card whose name has that in it, or `gpu=1` the second.

use std::sync::OnceLock;

use vulkano::{
    descriptor_set::layout::DescriptorType,
    device::physical::PhysicalDevice,
    format::{Format, FormatFeatures, NumericFormat},
    memory::{MemoryPropertyFlags, MemoryProperties},
    shader::{
        reflect,
        spirv::{Capability, ExecutionMode, Id, Instruction, Spirv, StorageClass},
    },
    swapchain::{ColorSpace, PresentMode},
};

/// What the game goes by, of what a graphics card can do
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Bytes of push constants
    pub push_constants: u32,
    /// Texels along a side of a 2D image, and of a 3D one
    pub image_2d: u32,
    pub image_3d: u32,
    /// Bytes of a storage buffer a shader can see at once, and in one allocation of memory
    pub storage_buffer_range: u64,
    pub allocation: u64,
    /// Storage images, storage buffers, uniform buffers and all of those a shader can use
    pub storage_images: u32,
    pub storage_buffers: u32,
    pub uniform_buffers: u32,
    pub resources: u32,
    /// Descriptor sets bound at once
    pub sets: u32,
    /// Bytes of memory shared by a workgroup, how many invocations it can have, and how big it can
    /// be along each axis
    pub shared_memory: u32,
    pub invocations: u32,
    pub workgroup: [u32; 3],
}

/// The least of what AMD's and Intel's graphics cards have under Windows, from an RX 470 and an
/// Iris Xe on: AMD's 128 bytes of push constants (also the Vulkan spec's minimum) and both's 2048
/// texels along a 3D image, Intel's 1 GB storage buffers, 16 storage images to a shader and 32 KB of
/// shared memory. Where neither is known to have less, the LunarG desktop baseline's, the least of
/// every desktop graphics card reported to vulkan.gpuinfo.org, and the spec's 1 GB allocations.
pub const MIN: Limits = Limits {
    push_constants: 128,
    image_2d: 16384,
    image_3d: 2048,
    storage_buffer_range: 1_073_741_820,
    allocation: 1 << 30,
    storage_images: 16,
    storage_buffers: 31,
    uniform_buffers: 15,
    resources: 159,
    sets: 8,
    shared_memory: 32768,
    invocations: 1024,
    workgroup: [1024, 1024, 64],
};

impl Limits {
    /// A graphics card's own
    pub fn of(physical: &PhysicalDevice) -> Self {
        let p = physical.properties();
        Limits {
            push_constants: p.max_push_constants_size,
            image_2d: p.max_image_dimension2_d,
            image_3d: p.max_image_dimension3_d,
            storage_buffer_range: p.max_storage_buffer_range as u64,
            // Vulkan 1.1 has every card say, and no less than this
            allocation: p.max_memory_allocation_size.unwrap_or(1 << 30),
            storage_images: p.max_per_stage_descriptor_storage_images,
            storage_buffers: p.max_per_stage_descriptor_storage_buffers,
            uniform_buffers: p.max_per_stage_descriptor_uniform_buffers,
            resources: p.max_per_stage_resources,
            sets: p.max_bound_descriptor_sets,
            shared_memory: p.max_compute_shared_memory_size,
            invocations: p.max_compute_work_group_invocations,
            workgroup: p.max_compute_work_group_size,
        }
    }

    /// No more than `other` of anything
    pub fn min(self, other: Limits) -> Self {
        Limits {
            push_constants: self.push_constants.min(other.push_constants),
            image_2d: self.image_2d.min(other.image_2d),
            image_3d: self.image_3d.min(other.image_3d),
            storage_buffer_range: self.storage_buffer_range.min(other.storage_buffer_range),
            allocation: self.allocation.min(other.allocation),
            storage_images: self.storage_images.min(other.storage_images),
            storage_buffers: self.storage_buffers.min(other.storage_buffers),
            uniform_buffers: self.uniform_buffers.min(other.uniform_buffers),
            resources: self.resources.min(other.resources),
            sets: self.sets.min(other.sets),
            shared_memory: self.shared_memory.min(other.shared_memory),
            invocations: self.invocations.min(other.invocations),
            workgroup: [0, 1, 2].map(|i| self.workgroup[i].min(other.workgroup[i])),
        }
    }

    /// The most bytes a buffer a shader sees whole can be: bound as one storage buffer, in one
    /// allocation, like the bricks of a world
    pub fn buffer_bytes(&self) -> u64 {
        self.storage_buffer_range.min(self.allocation)
    }

    /// What a shader needs more of than there is, said so a player can make sense of it, if
    /// anything
    pub fn shortfalls(&self, needs: &Needs) -> Vec<String> {
        let mut short = Vec::new();
        let mut over = |what: &str, needed: u64, has: u64| {
            if needed > has {
                short.push(format!("{needed} {what}, where the graphics card has room for {has}"));
            }
        };
        over("bytes of push constants", needs.push_constants as u64, self.push_constants as u64);
        over("storage images", needs.storage_images as u64, self.storage_images as u64);
        over("storage buffers", needs.storage_buffers as u64, self.storage_buffers as u64);
        over("uniform buffers", needs.uniform_buffers as u64, self.uniform_buffers as u64);
        over("resources", needs.resources() as u64, self.resources as u64);
        over("descriptor sets", needs.sets as u64, self.sets as u64);
        over("bytes of shared memory", needs.shared_memory as u64, self.shared_memory as u64);
        over("invocations a workgroup", needs.workgroup.iter().product::<u32>() as u64, self.invocations as u64);
        for i in 0..3 {
            over(["texels across a workgroup", "texels down a workgroup", "texels deep a workgroup"][i], needs.workgroup[i] as u64, self.workgroup[i] as u64);
        }
        // Neither is turned on, see Gpu::new() in main.rs
        if needs.formatless_images > 0 {
            short.push(format!("{} storage images without a format, which needs shaderStorageImageReadWithoutFormat", needs.formatless_images));
        }
        short
    }
}

static LIMITS: OnceLock<Limits> = OnceLock::new();

/// `--set limits=min`, see the top of this file
pub fn min_mode() -> bool {
    crate::settings_from_args().iter().any(|(k, v)| k == "limits" && v == "min")
}

/// Goes by `physical`'s limits from now on, no more than MIN with `--set limits=min`
pub fn set(physical: &PhysicalDevice) -> Limits {
    let card = Limits::of(physical);
    let limits = if min_mode() { card.min(MIN) } else { card };
    let _ = LIMITS.set(limits);
    limits
}

/// What the game goes by, see set(): MIN until the graphics card's picked
pub fn get() -> Limits {
    LIMITS.get().copied().unwrap_or(MIN)
}

/// What a compute shader needs, as found in its SPIR-V
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Needs {
    /// Bytes of push constants, to the end of the last
    pub push_constants: u32,
    pub storage_images: u32,
    pub storage_buffers: u32,
    pub uniform_buffers: u32,
    /// Anything else bound
    pub other_descriptors: u32,
    /// One more than the highest descriptor set
    pub sets: u32,
    /// Bytes of workgroup shared memory
    pub shared_memory: u32,
    pub workgroup: [u32; 3],
    /// The formats the storage images are read and written as, and how many have none given
    pub image_formats: Vec<Format>,
    pub formatless_images: u32,
    /// Whether any atomics are done on storage images
    pub image_atomics: bool,
    pub capabilities: Vec<Capability>,
}

impl Needs {
    /// Every resource a shader stage uses, as maxPerStageResources counts them
    pub fn resources(&self) -> u32 {
        self.storage_images + self.storage_buffers + self.uniform_buffers + self.other_descriptors
    }

    /// Of the compute shader `words`
    pub fn of(words: &[u32]) -> Result<Self, String> {
        let spirv = Spirv::new(words).map_err(|e| format!("not SPIR-V: {e}"))?;
        let (entry, info) = reflect::entry_points(&spirv).next().ok_or("no entry point")?;
        let mut needs = Needs {
            push_constants: info.push_constant_requirements.map_or(0, |r| r.offset + r.size),
            ..Default::default()
        };
        for (&(set, _), binding) in &info.descriptor_binding_requirements {
            let count = binding.descriptor_count.unwrap_or(1);
            needs.sets = needs.sets.max(set + 1);
            let is = |ty: DescriptorType| binding.descriptor_types.contains(&ty);
            if is(DescriptorType::StorageImage) {
                needs.storage_images += count;
                match binding.image_format {
                    Some(format) => needs.image_formats.push(format),
                    None => needs.formatless_images += count,
                }
                needs.image_atomics |= binding.descriptors.values().any(|d| d.storage_image_atomic);
            } else if is(DescriptorType::StorageBuffer) {
                needs.storage_buffers += count;
            } else if is(DescriptorType::UniformBuffer) {
                needs.uniform_buffers += count;
            } else {
                needs.other_descriptors += count;
            }
        }
        needs.image_formats.sort_by_key(|f| *f as u32);
        needs.image_formats.dedup();
        for instruction in spirv.execution_modes() {
            if let Instruction::ExecutionMode { entry_point, mode: ExecutionMode::LocalSize { x_size, y_size, z_size } } = instruction
                && *entry_point == entry
            {
                needs.workgroup = [*x_size, *y_size, *z_size];
            }
        }
        for instruction in spirv.global_variables() {
            if let Instruction::Variable { result_type_id, storage_class: StorageClass::Workgroup, .. } = instruction
                && let Instruction::TypePointer { ty, .. } = spirv.id(*result_type_id).instruction()
            {
                needs.shared_memory += size_of_type(&spirv, *ty);
            }
        }
        needs.capabilities = spirv
            .capabilities()
            .iter()
            .filter_map(|c| match c {
                Instruction::Capability { capability } => Some(*capability),
                _ => None,
            })
            .collect();
        Ok(needs)
    }
}

/// Bytes a value of SPIR-V type `ty` takes in workgroup memory, packed tightly, as near as matters
fn size_of_type(spirv: &Spirv, ty: Id) -> u32 {
    match spirv.id(ty).instruction() {
        Instruction::TypeBool { .. } => 4,
        Instruction::TypeInt { width, .. } | Instruction::TypeFloat { width, .. } => width / 8,
        Instruction::TypeVector { component_type, component_count, .. } => size_of_type(spirv, *component_type) * component_count,
        Instruction::TypeMatrix { column_type, column_count, .. } => size_of_type(spirv, *column_type) * column_count,
        Instruction::TypeArray { element_type, length, .. } => {
            let length = match spirv.id(*length).instruction() {
                Instruction::Constant { value, .. } => value.first().copied().unwrap_or(0),
                _ => 0,
            };
            size_of_type(spirv, *element_type) * length
        }
        Instruction::TypeStruct { member_types, .. } => member_types.iter().map(|&m| size_of_type(spirv, m)).sum(),
        _ => 0,
    }
}

/// Every format the game's shaders read and write storage images as, and the images it copies,
/// clears and blits, with what's done to them: all of them ones every Vulkan driver must support
/// for storage, but each is checked, see unsuitable()
pub const IMAGE_FORMATS: &[(Format, FormatFeatures)] = &[
    // Rendered into, copied from for screenshots and blitted to the window
    (Format::R8G8B8A8_UNORM, FormatFeatures::STORAGE_IMAGE.union(FormatFeatures::TRANSFER_SRC).union(FormatFeatures::TRANSFER_DST).union(FormatFeatures::BLIT_SRC)),
    (Format::R16G16B16A16_SFLOAT, FormatFeatures::STORAGE_IMAGE),
    (Format::R32G32B32A32_SFLOAT, FormatFeatures::STORAGE_IMAGE),
    // Depths, copied from to see how much of the view is sky, and the water's
    (Format::R32_SFLOAT, FormatFeatures::STORAGE_IMAGE.union(FormatFeatures::TRANSFER_SRC).union(FormatFeatures::TRANSFER_DST)),
    // Brick grids and the grass map, sent over, and the map of where sunlight stops, cleared
    (Format::R32_UINT, FormatFeatures::STORAGE_IMAGE.union(FormatFeatures::TRANSFER_DST)),
];

/// Why the game can't run on `physical`, if it can't: what it's missing of what's not certain to
/// be there, as best can be told before the shaders are made, see Limits::shortfalls() for those
pub fn unsuitable(physical: &PhysicalDevice) -> Option<String> {
    let mut missing = Vec::new();
    for &(format, features) in IMAGE_FORMATS {
        let has = physical.format_properties(format).map(|p| p.optimal_tiling_features).unwrap_or_default();
        if !has.contains(features) {
            missing.push(format!("{format:?} images it can {:?}", features.difference(has)));
        }
    }
    let limits = Limits::of(physical);
    // Even the narrowest piece of the landscape's grid of bricks, and maps of its columns
    let side = crate::memory::WIDTHS_M.last().unwrap() * crate::INITIAL_TERRAIN.voxels_per_meter;
    if limits.image_3d < side / 8 || limits.image_2d < side {
        missing.push(format!("images {side} texels across"));
    }
    (!missing.is_empty()).then(|| missing.join(", "))
}

/// The memory types the game allocates from: with `--set limits=min`, none of the graphics card's
/// own the CPU can write to, where there's other memory it can, as on an AMD card without Resizable
/// BAR once its 256 MB window of them is used up. What's then written by the CPU and read by the
/// GPU goes in the computer's memory, which the GPU reads over PCI Express.
pub fn memory_types(properties: &MemoryProperties, mut bits: u32) -> u32 {
    if !min_mode() {
        return bits;
    }
    let host_only = properties.memory_types.iter().any(|t| t.property_flags.contains(MemoryPropertyFlags::HOST_VISIBLE) && !t.property_flags.intersects(MemoryPropertyFlags::DEVICE_LOCAL));
    if host_only {
        for (i, t) in properties.memory_types.iter().enumerate() {
            if t.property_flags.contains(MemoryPropertyFlags::DEVICE_LOCAL | MemoryPropertyFlags::HOST_VISIBLE) {
                bits &= !(1 << i);
            }
        }
    }
    bits
}

/// How frames are shown, of those the window has: as soon as they're drawn, Immediate, which every
/// card here has but isn't certain; else Mailbox, which also never waits for the screen; else Fifo,
/// which every driver has
pub fn present_mode(supported: &[PresentMode]) -> PresentMode {
    [PresentMode::Immediate, PresentMode::Mailbox].into_iter().find(|m| supported.contains(m)).unwrap_or(PresentMode::Fifo)
}

/// The format the window's images are drawn in, of those it has: 8 bits a channel in sRGB, which
/// post.comp encodes for when the format doesn't, see POST_ENCODE_SRGB, and which the rendered image
/// can be blitted to (`blits`). The first that can be blitted to otherwise, else the first.
pub fn surface_format(supported: &[(Format, ColorSpace)], blits: impl Fn(Format) -> bool) -> (Format, ColorSpace) {
    let usual = |f: Format| matches!(f, Format::B8G8R8A8_UNORM | Format::R8G8B8A8_UNORM | Format::B8G8R8A8_SRGB | Format::R8G8B8A8_SRGB | Format::A8B8G8R8_UNORM_PACK32 | Format::A8B8G8R8_SRGB_PACK32);
    let srgb_space = |&(f, c): &(Format, ColorSpace)| c == ColorSpace::SrgbNonLinear && blits(f);
    // UNORM first, as the game has always picked where it was first
    let unorm = |&(f, _): &(Format, ColorSpace)| f.numeric_format_color() == Some(NumericFormat::UNORM);
    supported
        .iter()
        .find(|s| srgb_space(s) && usual(s.0) && unorm(s))
        .or_else(|| supported.iter().find(|s| srgb_space(s) && usual(s.0)))
        .or_else(|| supported.iter().find(|s| srgb_space(s)))
        .or_else(|| supported.iter().find(|s| blits(s.0)))
        .or(supported.first())
        .copied()
        .unwrap_or((Format::B8G8R8A8_UNORM, ColorSpace::SrgbNonLinear))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shader of the game, compiled
    fn shaders() -> Vec<(&'static str, Needs)> {
        crate::Pipelines::SHADERS
            .iter()
            .map(|&(name, source)| {
                let words = crate::hot_reload::compile_source(source, std::path::Path::new(name), shaderc::ShaderKind::Compute, "main").unwrap_or_else(|e| panic!("{name}: {e}"));
                (name, Needs::of(words.as_binary()).unwrap_or_else(|e| panic!("{name}: {e}")))
            })
            .collect()
    }

    // Every pipeline can be made on AMD's and Intel's cards: 128 bytes of push constants at most,
    // and no more of anything else than they have. `cargo test --release limits -- --nocapture`
    // says how much each needs.
    #[test]
    fn every_shader_fits_amd_and_intel() {
        let mut short = Vec::new();
        for (name, needs) in shaders() {
            println!(
                "{name}: {} bytes of push constants, {} storage images, {} storage buffers, {} uniform buffers, {} sets, {} bytes shared, workgroup {:?}",
                needs.push_constants, needs.storage_images, needs.storage_buffers, needs.uniform_buffers, needs.sets, needs.shared_memory, needs.workgroup
            );
            short.extend(MIN.shortfalls(&needs).into_iter().map(|s| format!("{name} needs {s}")));
        }
        assert!(short.is_empty(), "{}", short.join("\n"));
    }

    // Nothing the game doesn't turn on, see Gpu::new() in main.rs: every storage image has a
    // format given, in one every driver must be able to use for storage, no atomics on images and
    // no 8, 16 or 64 bit numbers
    #[test]
    fn shaders_need_nothing_turned_on() {
        let storage_formats: Vec<Format> = IMAGE_FORMATS.iter().map(|(f, _)| *f).collect();
        for (name, needs) in shaders() {
            assert_eq!(needs.formatless_images, 0, "{name}");
            assert!(!needs.image_atomics, "{name}");
            for format in &needs.image_formats {
                assert!(storage_formats.contains(format), "{name} uses {format:?} storage images, which isn't checked for");
            }
            for capability in &needs.capabilities {
                assert!(
                    matches!(capability, Capability::Shader | Capability::ImageQuery),
                    "{name} needs {capability:?}, which may need a device feature turned on"
                );
            }
        }
    }

    #[test]
    fn present_modes_fall_back() {
        use PresentMode::*;
        assert_eq!(present_mode(&[Fifo, FifoRelaxed, Mailbox, Immediate]), Immediate);
        // As on Intel's integrated graphics and older AMD drivers
        assert_eq!(present_mode(&[Immediate, Fifo]), Immediate);
        assert_eq!(present_mode(&[Fifo, Mailbox]), Mailbox);
        assert_eq!(present_mode(&[Fifo]), Fifo);
        assert_eq!(present_mode(&[]), Fifo);
    }

    #[test]
    fn surface_formats_suit_the_blit() {
        let all = |_| true;
        let srgb = ColorSpace::SrgbNonLinear;
        // NVIDIA, AMD and Intel under Windows all list this first
        assert_eq!(surface_format(&[(Format::B8G8R8A8_UNORM, srgb), (Format::B8G8R8A8_SRGB, srgb)], all), (Format::B8G8R8A8_UNORM, srgb));
        // Ten bit or HDR ones first aren't taken over 8 bit sRGB
        assert_eq!(surface_format(&[(Format::A2B10G10R10_UNORM_PACK32, srgb), (Format::B8G8R8A8_SRGB, srgb)], all), (Format::B8G8R8A8_SRGB, srgb));
        assert_eq!(surface_format(&[(Format::R16G16B16A16_SFLOAT, ColorSpace::ExtendedSrgbLinear), (Format::R8G8B8A8_UNORM, srgb)], all), (Format::R8G8B8A8_UNORM, srgb));
        // Nor ones that can't be blitted to
        assert_eq!(surface_format(&[(Format::B8G8R8A8_UNORM, srgb), (Format::R8G8B8A8_UNORM, srgb)], |f| f == Format::R8G8B8A8_UNORM), (Format::R8G8B8A8_UNORM, srgb));
        assert_eq!(surface_format(&[(Format::A2B10G10R10_UNORM_PACK32, srgb)], all), (Format::A2B10G10R10_UNORM_PACK32, srgb));
    }

    #[test]
    fn min_is_no_more_than_any_card() {
        let nvidia = Limits { push_constants: 256, image_3d: 16384, storage_buffer_range: u32::MAX as u64, ..MIN };
        assert_eq!(nvidia.min(MIN), MIN);
        assert_eq!(MIN.min(nvidia), MIN);
        // Every pipeline's checked against no less than the Vulkan spec's minimum push constants
        assert!(MIN.push_constants >= 128);
    }
}
