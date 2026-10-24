use std::{collections::HashSet, env, fmt::Write, fs, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=resources");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let header = fs::read_to_string("resources/ids.h").expect("read resource IDs");
    let mut names = HashSet::new();
    let mut numbers = HashSet::new();
    let mut rust = String::from("// Generated from resources/ids.h by build.rs.\n");
    for line in header.lines().filter(|line| line.starts_with("#define ")) {
        let words: Vec<_> = line.split_whitespace().collect();
        assert_eq!(words.len(), 3, "resource IDs must be numeric defines");
        let name = words[1];
        assert!(
            ["IDD_", "IDC_", "IDR_", "IDM_"]
                .iter()
                .any(|prefix| name.starts_with(prefix)),
            "unsupported resource ID: {name}"
        );
        let value: u16 = words[2].parse().expect("resource IDs must fit u16");
        assert!(value > 2, "IDOK and IDCANCEL are reserved");
        assert!(names.insert(name), "duplicate resource name: {name}");
        assert!(numbers.insert(value), "duplicate resource ID: {value}");
        writeln!(rust, "pub const {name}: u16 = {value};").unwrap();
    }
    fs::write(output.join("ids.rs"), rust).expect("write generated IDs");

    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = ["MAJOR", "MINOR", "PATCH"]
        .map(|part| env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap());
    fs::write(
        output.join("version.h"),
        format!(
            "#define NETMAP_VERSION \"{version}\"\n#define NETMAP_VERSION_NUMBERS {},{},{},0\n",
            numbers[0], numbers[1], numbers[2]
        ),
    )
    .expect("write resource version");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("resources/netmap.rc", embed_resource::NONE)
            .manifest_required()
            .expect("compile Windows resources with the Windows SDK");
    }
}
