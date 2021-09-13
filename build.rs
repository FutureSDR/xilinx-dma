fn main() {
    cc::Build::new()
        .file("dmb.c")
        // Overrride -march=armv6 coming from cross,
        // since it gives the error:
        // selected processor does not support `dmb sy' in ARM mode
        .flag("-march=armv7-a")
        .compile("dmb");
}
