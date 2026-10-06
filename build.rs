use std::{env, path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=tailwind.input.css");
    println!("cargo:rerun-if-env-changed=TAILWINDCSS");

    let input = manifest_dir.join("tailwind.input.css");
    let output = manifest_dir.join("static/styles.css");

    let bin = env::var("TAILWINDCSS").unwrap_or("tailwindcss".into());
    let mut cmd = Command::new(&bin);
    cmd.current_dir(&manifest_dir)
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output);
    if env::var("PROFILE").as_deref() == Ok("release") {
        cmd.arg("--minify");
    }

    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("failed to run `{bin}` (is the Tailwind CLI installed?): {e}"));
    assert!(status.success(), "tailwindcss exited with {status}");
}
