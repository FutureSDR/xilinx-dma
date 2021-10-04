use std::env;

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH")
        .expect("Env variable CARGO_CFG_TARGET_ARCH not found");
    
    if arch != "arm" && arch != "aarch64" {
        // dmb is only available in ARM
        return;
    }
    
    let mut build = cc::Build::new();
    build.file("dmb.c");
    if arch == "arm" {
        // Overrride -march=armv6 coming from cross,
        // since it gives the error:
        // selected processor does not support `dmb sy' in ARM mode
        build.flag("-march=armv7-a");
    }
    build.compile("dmb");
}
