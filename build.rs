// The game and the editor each run on a main thread whose stack Windows sets at link time, 1 MB
// unless told otherwise. A debug build's unoptimised code needs far more than that while it starts
// (it overflowed as soon as the graphics card was up), so both programs ask for 64 MB. That's only
// reserved address space; the memory's only taken as the stack really grows into it.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:67108864");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
