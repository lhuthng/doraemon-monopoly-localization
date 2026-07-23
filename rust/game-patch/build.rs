use std::{env, fs, path::PathBuf, process::Command};

fn run(command: &mut Command, description: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("start {description}: {error}"));
    assert!(status.success(), "{description} failed with {status}");
}

fn main() {
    println!("cargo:rerun-if-changed=src/pe/bgm_runtime.c");
    println!("cargo:rerun-if-changed=src/pe/bgm_runtime.ld");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("bgm_runtime.o");
    let image = out.join("bgm_runtime.exe");
    let binary = out.join("bgm_runtime.bin");
    run(
        Command::new("i686-w64-mingw32-gcc")
            .args([
                "-Os",
                "-march=i386",
                "-mtune=i386",
                "-ffreestanding",
                "-fno-builtin",
                "-fno-stack-protector",
                "-fno-asynchronous-unwind-tables",
                "-fno-ident",
                "-fdata-sections",
                "-ffunction-sections",
                "-c",
            ])
            .arg(root.join("src/pe/bgm_runtime.c"))
            .arg("-o")
            .arg(&object),
        "BGM runtime compilation",
    );
    run(
        Command::new("i686-w64-mingw32-gcc")
            .args([
                "-nostdlib",
                "-Wl,--entry,_BgmInit@4",
                "-Wl,--gc-sections",
                "-Wl,-T",
            ])
            .arg(root.join("src/pe/bgm_runtime.ld"))
            .arg("-o")
            .arg(&image)
            .arg(&object),
        "BGM runtime link",
    );
    run(
        Command::new("i686-w64-mingw32-objcopy")
            .args(["-j", ".bgm", "-O", "binary"])
            .arg(&image)
            .arg(&binary),
        "BGM runtime extraction",
    );
    let nm = Command::new("i686-w64-mingw32-nm")
        .arg("-n")
        .arg(&image)
        .output()
        .expect("start BGM symbol reader");
    assert!(nm.status.success(), "BGM symbol reader failed");
    let symbols = String::from_utf8(nm.stdout).expect("BGM symbols are UTF-8");
    let wanted = ["BgmDispatch"];
    let mut generated = String::new();
    for name in wanted {
        let address = symbols
            .lines()
            .find_map(|line| {
                let mut fields = line.split_whitespace();
                let address = fields.next()?;
                let _kind = fields.next()?;
                let symbol = fields.next()?;
                let normalized = symbol
                    .trim_start_matches('_')
                    .split('@')
                    .next()
                    .unwrap_or(symbol);
                (normalized == name).then_some(address)
            })
            .unwrap_or_else(|| panic!("missing BGM runtime symbol {name}"));
        generated.push_str(&format!(
            "pub const {}: u32 = 0x{};\n",
            name.to_ascii_uppercase(),
            address
        ));
    }
    fs::write(out.join("bgm_symbols.rs"), generated).expect("write BGM symbols");
}
