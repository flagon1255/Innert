use std::{
    fs::File,
    io::Read,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use vulkano::{
    device::{Device, DeviceOwned},
    pipeline::{
        ComputePipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo,
        compute::ComputePipelineCreateInfo, layout::PipelineDescriptorSetLayoutCreateInfo,
    },
    shader::{ShaderModule, ShaderModuleCreateInfo},
};

/// The shader at `path`, or if it isn't there, as on another computer than the one it was built
/// on, the copy built into the program
fn source(path: &Path, built_in: &str) -> String {
    let mut source_text = String::new();
    match File::open(path) {
        Ok(mut f) => {
            f.read_to_string(&mut source_text).unwrap();
        }
        Err(_) => source_text = built_in.to_owned(),
    }
    source_text
}

pub(crate) fn compile_source(
    source_text: &str,
    path: &Path,
    kind: shaderc::ShaderKind,
    entry_point_name: &str,
) -> Result<shaderc::CompilationArtifact, shaderc::Error> {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    options.add_macro_definition("EP", Some(entry_point_name));
    compiler.compile_into_spirv(
        source_text,
        kind,
        path.to_str().unwrap(),
        entry_point_name,
        Some(&options),
    )
}

/// Compiled shaders are kept here from one run to the next: compiling traverse.comp takes the best
/// part of a quarter of a minute, which every start of the game used to wait for
fn cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Innert")
        .join("shaders")
}

/// Starts each kept shader's file, and the version of how it's kept
const CACHE_MAGIC: &[u8; 8] = b"INNERT\0\x01";
/// The most compiled versions of each shader kept, the latest, as when trying out changes to one
const CACHE_KEPT: usize = 4;
/// The first word of all SPIR-V
const SPIRV_MAGIC: u32 = 0x0723_0203;

/// FNV-1a, which hashes the same in every build of the game, unlike std's
fn fnv(parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for part in parts {
        for &byte in *part {
            hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
        // So ["ab", "c"] and ["a", "bc"] differ
        hash = (hash ^ 0xff).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Where the shader compiled from `source_text` is kept: everything that goes into compiling it is
/// in its name, the compiler's version too, so a changed shader is never mistaken for the one before
fn cache_file(path: &Path, source_text: &str, kind: shaderc::ShaderKind, entry_point_name: &str) -> PathBuf {
    let how = format!("{kind:?} {entry_point_name} optimized for performance, SPIR-V {:?}", shaderc::get_spirv_version());
    let key = fnv(&[how.as_bytes(), source_text.as_bytes()]);
    let name = path.file_name().map_or("shader".into(), |n| n.to_string_lossy());
    cache_dir().join(format!("{name}-{key:016x}.spv"))
}

/// The SPIR-V kept in `file`, if it's there and whole
fn read_cached(file: &Path) -> Option<Vec<u32>> {
    let bytes = std::fs::read(file).ok()?;
    let (header, body) = bytes.split_at_checked(16)?;
    if &header[..8] != CACHE_MAGIC || body.len() % 4 != 0 || u64::from_le_bytes(header[8..].try_into().ok()?) != fnv(&[body]) {
        return None;
    }
    let words: Vec<u32> = body.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect();
    (words.first() == Some(&SPIRV_MAGIC)).then_some(words)
}

/// Keeps `words` in `file` for next time, and lets go of all but the latest few kept of the same
/// shader. Where it can't be kept, it's compiled again next time.
fn write_cached(file: &Path, words: &[u32]) {
    let Some(dir) = file.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    let body: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut bytes = CACHE_MAGIC.to_vec();
    bytes.extend(fnv(&[&body]).to_le_bytes());
    bytes.extend(body);
    // Written aside then moved into place, so another run of the game starting meanwhile never
    // reads half of it
    let aside = file.with_extension(format!("{}.part", std::process::id()));
    if std::fs::write(&aside, bytes).is_err() || std::fs::rename(&aside, file).is_err() {
        let _ = std::fs::remove_file(&aside);
        return;
    }
    let Some(name) = file.file_name().and_then(|n| n.to_str()).and_then(|n| n.rsplit_once('-')).map(|(name, _)| format!("{name}-")) else {
        return;
    };
    let mut kept: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with(&name) && n.ends_with(".spv")))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    kept.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, old) in kept.into_iter().skip(CACHE_KEPT) {
        let _ = std::fs::remove_file(old);
    }
}

/// Whether the shader at `path` was compiled before as it is now, so getting it is quick
pub fn is_cached(path: &Path, built_in: &str) -> bool {
    cache_file(path, &source(path, built_in), shaderc::ShaderKind::Compute, "main").exists()
}

/// The shader at `path` compiled, as it was kept from before if it was, see cache_dir()
fn spirv(path: &Path, built_in: &str, kind: shaderc::ShaderKind, entry_point_name: &str) -> Result<Vec<u32>, shaderc::Error> {
    let source_text = source(path, built_in);
    let file = cache_file(path, &source_text, kind, entry_point_name);
    if let Some(words) = read_cached(&file) {
        return Ok(words);
    }
    let words = compile_source(&source_text, path, kind, entry_point_name)?.as_binary().to_vec();
    write_cached(&file, &words);
    Ok(words)
}

/// What the shader `words` from `path` needs more of than the graphics card has, see limits.rs,
/// if anything: 128 bytes of push constants on AMD's cards, say, where NVIDIA's have 256
fn shortfalls(path: &Path, words: &[u32]) -> Option<String> {
    let name = path.file_name().map_or("a shader".into(), |n| n.to_string_lossy());
    let short = match crate::limits::Needs::of(words) {
        Ok(needs) => crate::limits::get().shortfalls(&needs),
        Err(e) => vec![e],
    };
    (!short.is_empty()).then(|| format!("{name} needs {}", short.join(", and ")))
}

fn get_pipeline(shader_module: Arc<ShaderModule>) -> Arc<ComputePipeline> {
    let device = shader_module.device().clone();
    let entry_point = shader_module.entry_point("main").unwrap();

    let stage = PipelineShaderStageCreateInfo::new(entry_point);

    let layout = PipelineLayout::new(
        device.clone(),
        PipelineDescriptorSetLayoutCreateInfo::from_stages([&stage])
            .into_pipeline_layout_create_info(device.clone())
            .unwrap(),
    )
    .unwrap();

    ComputePipeline::new(
        device.clone(),
        None,
        ComputePipelineCreateInfo::stage_layout(stage, layout),
    )
    .unwrap()
}

pub struct HotReloadComputePipeline {
    pipeline: Arc<ComputePipeline>,
    reload: Arc<AtomicBool>,
    path: PathBuf,
    built_in: &'static str,
    // Only when the shader's file is there to watch
    _watcher: Option<RecommendedWatcher>,
}

impl Deref for HotReloadComputePipeline {
    type Target = Arc<ComputePipeline>;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl HotReloadComputePipeline {
    /// The shader at `path`, reloaded whenever it changes, or `built_in` if there's no such file
    pub fn new(device: Arc<Device>, path: &Path, built_in: &'static str) -> Self {
        let reload = Arc::<AtomicBool>::default();
        let cloned_reload = reload.clone();
        let watcher = path.exists().then(|| {
            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    if let Ok(event) = res
                        && event.kind.is_modify()
                    {
                        cloned_reload.store(true, Ordering::Relaxed);
                    }
                })
                .unwrap();
            watcher.watch(path, RecursiveMode::NonRecursive).unwrap();
            watcher
        });

        let words = spirv(path, built_in, shaderc::ShaderKind::Compute, "main").unwrap();
        // Said plainly where the graphics card hasn't what it takes, rather than falling over
        if let Some(short) = shortfalls(path, &words) {
            crate::fail(&format!("This graphics card can't run Innert: {short}."));
        }

        let shader_module = unsafe {
            ShaderModule::new(device, ShaderModuleCreateInfo::new(&words)).unwrap()
        };

        let pipeline = get_pipeline(shader_module);

        Self {
            pipeline,
            reload,
            path: path.to_path_buf(),
            built_in,
            _watcher: watcher,
        }
    }

    /// Returns true if the pipeline was replaced.
    pub fn maybe_reload(&mut self) -> bool {
        if self.reload.swap(false, Ordering::Relaxed) {
            let words = match spirv(&self.path, self.built_in, shaderc::ShaderKind::Compute, "main") {
                Ok(words) => words,
                Err(e) => {
                    eprint!("{}", e);
                    return false;
                }
            };
            if let Some(short) = shortfalls(&self.path, &words) {
                eprintln!("{short}");
                return false;
            }

            let shader_module = unsafe {
                ShaderModule::new(
                    self.pipeline.device().clone(),
                    ShaderModuleCreateInfo::new(&words),
                )
                .unwrap()
            };

            let new_pipeline = get_pipeline(shader_module);

            let num_sets = self.pipeline.layout().set_layouts().len() as u32;
            let new_num_sets = new_pipeline.layout().set_layouts().len() as u32;
            if num_sets != new_num_sets
                || !new_pipeline
                    .layout()
                    .is_compatible_with(self.pipeline.layout(), num_sets)
            {
                eprintln!(
                    "{} layout is not compatible with pipeline.",
                    self.path.display()
                );
                return false;
            }

            self.pipeline = new_pipeline;
            eprintln!("{} successfully reloaded", self.path.display());
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // As on another computer, where the shaders folder it was built from isn't there
    #[test]
    fn built_in_shaders_compile_without_their_files() {
        let missing = Path::new("no such folder");
        for (name, built_in) in [
            ("traverse.comp", include_str!("../shaders/traverse.comp")),
            ("post.comp", include_str!("../shaders/post.comp")),
            ("resample.comp", include_str!("../shaders/resample.comp")),
            ("upscale.comp", include_str!("../shaders/upscale.comp")),
            ("water.comp", include_str!("../shaders/water.comp")),
            ("shafts.comp", include_str!("../shaders/shafts.comp")),
        ] {
            let path = missing.join(name);
            if let Err(e) = compile_source(&source(&path, built_in), &path, shaderc::ShaderKind::Compute, "main") {
                panic!("{name}: {e}");
            }
        }
    }

    // What's kept comes back as it went in, and anything else isn't taken for it
    #[test]
    fn kept_shaders_come_back_whole() {
        let dir = std::env::temp_dir().join(format!("innert-shader-cache-test-{}", std::process::id()));
        let file = dir.join("test.comp-0123456789abcdef.spv");
        let words = vec![SPIRV_MAGIC, 0x0001_0300, 7, 42, u32::MAX];
        write_cached(&file, &words);
        assert_eq!(read_cached(&file), Some(words));

        // Cut short, as if the computer went off while it was written
        let bytes = std::fs::read(&file).unwrap();
        std::fs::write(&file, &bytes[..bytes.len() - 4]).unwrap();
        assert_eq!(read_cached(&file), None);
        // Changed
        let mut changed = bytes.clone();
        *changed.last_mut().unwrap() ^= 1;
        std::fs::write(&file, &changed).unwrap();
        assert_eq!(read_cached(&file), None);
        assert_eq!(read_cached(&dir.join("not there.spv")), None);

        // Only the latest few versions of a shader are kept
        for k in 0..CACHE_KEPT + 2 {
            write_cached(&dir.join(format!("test.comp-{k:016x}.spv")), &[SPIRV_MAGIC, k as u32]);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let left = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(left, CACHE_KEPT);
    }

    // A shader changed in any way is kept apart from how it was
    #[test]
    fn changed_shaders_are_kept_apart() {
        let path = Path::new("shaders").join("water.comp");
        let kind = shaderc::ShaderKind::Compute;
        let a = cache_file(&path, "void main() {}", kind, "main");
        assert_eq!(a, cache_file(&path, "void main() {}", kind, "main"));
        assert_ne!(a, cache_file(&path, "void main() { }", kind, "main"));
        assert_ne!(a, cache_file(&path, "void main() {}", kind, "other"));
        assert_ne!(a, cache_file(&Path::new("shaders").join("post.comp"), "void main() {}", kind, "main"));
    }
}
