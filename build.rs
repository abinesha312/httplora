fn main() {
    // Build C++ radio HAL using CMake
    let dst = cmake::Config::new("radio")
        .build();
    
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=radio_hal");
    
    // Link C++ standard library
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
    
    // Build cxx bridge
    cxx_build::bridge("src/radio_ffi.rs")
        .file("src/radio_bridge.cpp")
        .include("radio")
        .flag_if_supported("-std=c++17")
        .compile("radio_bridge");
    
    println!("cargo:rerun-if-changed=src/radio_ffi.rs");
    println!("cargo:rerun-if-changed=src/radio_bridge.cpp");
    println!("cargo:rerun-if-changed=radio/");
}
