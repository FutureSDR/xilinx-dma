use std::env;

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("Env variable CARGO_CFG_TARGET_ARCH not found");
    
    if arch != "armv7" && arch != "aarch64" {
        // dmb is only available in armv7 and aarch64
        return;
    }
    
    let mut build = cc::Build::new();
    build.file("dmb.c");
    build.compile("dmb");
}
