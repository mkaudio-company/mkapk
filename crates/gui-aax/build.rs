fn main() {
    println!("cargo:rerun-if-env-changed=AAX_SDK");
    println!("cargo:rustc-check-cfg=cfg(aax_sdk)");
    match std::env::var("AAX_SDK") {
        Ok(path) => {
            println!("cargo:rustc-cfg=aax_sdk");
            println!("cargo:rustc-link-search=native={}/Libs", path);
        }
        Err(_) => {
            println!("cargo:warning=AAX_SDK not set; building gui-aax as a no-op stub.");
        }
    }
}
