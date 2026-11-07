use std::process::Command;

fn git(a: &[&str]) -> String {
    Command::new("git").args(a).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

fn main() {
    let rev = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain", "--untracked-files=no"]).is_empty();
    let date = git(&["log", "-1", "--format=%cs"]);
    println!("cargo:rustc-env=NONAME_REV={rev}");
    println!("cargo:rustc-env=NONAME_DIRTY={}", if dirty { "1" } else { "" });
    println!("cargo:rustc-env=NONAME_DATE={date}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=src");
}
