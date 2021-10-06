use std::env;

fn main() {
    let target = env::var("TARGET").expect("Env variable TARGET not found");

    // dmb is only available in armv7 and aarch64
    if target.starts_with("armv7") || target.starts_with("aarch64") {
        let mut build = cc::Build::new();
        build.file("dmb.c");
        build.compile("dmb");

        println!("cargo:rustc-cfg=xilinx_dma_has_dmb");
        println!("cargo:rerun-if-changed=dmb.c");
    }
}
