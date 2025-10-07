use std::process::Command;

fn main() {
    // Get build date/time
    let build_date = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    // Get git commit hash
    let commit =
        get_git_output(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_COMMIT={}", commit);

    // Get git branch name
    let branch = get_git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_BRANCH={}", branch);

    // Rerun if git HEAD changes
    println!("cargo:rerun-if-changed=../.git/HEAD");
}

fn get_git_output(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}
