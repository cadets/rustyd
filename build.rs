fn main() {
    println!("cargo:rustc-link-search=native=/usr/local/lib");
    println!("cargo:rustc-link-lib=dylib=dtrace");
    println!("cargo:rustc-link-lib=dylib=proc");
    println!("cargo:rustc-link-lib=dylib=rtld_db");
    println!("cargo:rustc-link-lib=dylib=ctf");
    println!("cargo:rustc-link-lib=dylib=elf");
    println!("cargo:rustc-link-lib=dylib=c");
    println!("cargo:rustc-link-lib=dylib=z");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=util");
    println!("cargo:rustc-link-lib=dylib=xo");
}
