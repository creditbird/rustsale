//! Turns the demo exports on when RUSTSALE_DEMO=1 is set while building (tests and screenshots only).

fn main() {
    println!("cargo:rustc-check-cfg=cfg(rustsale_demo)");
    println!("cargo:rerun-if-env-changed=RUSTSALE_DEMO");
    if std::env::var("RUSTSALE_DEMO").is_ok_and(|v| v == "1") {
        println!("cargo:rustc-cfg=rustsale_demo");
    }
}
