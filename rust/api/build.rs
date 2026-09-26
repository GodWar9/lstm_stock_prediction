use std::{env, fs, path::Path};

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create embedded asset directory");
    for entry in fs::read_dir(source).expect("read frontend assets") {
        let entry = entry.expect("asset entry");
        let dest = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).expect("copy frontend asset");
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=assets/index.html");
    let target = std::path::PathBuf::from(env::var("OUT_DIR").unwrap()).join("assets");
    let source = if Path::new("../../web/dist/index.html").is_file() {
        "../../web/dist"
    } else {
        "assets"
    };
    copy_tree(Path::new(source), &target);
}
