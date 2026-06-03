/// Ensure `web/dist/` exists so rust-embed compiles even before `npm run build`.
/// The real build artefacts are written by `cd web && npm run build`.
fn main() {
    let dist = std::path::Path::new("web/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).expect("cannot create web/dist");
        std::fs::write(
            dist.join("index.html"),
            "<!DOCTYPE html><html><body>\
             <h1>Reqbook UI not built</h1>\
             <p>Run <code>cd web &amp;&amp; npm run build</code> then restart <code>rqb serve</code>.</p>\
             </body></html>",
        )
        .expect("cannot write placeholder index.html");
    }
    // Re-run if the web source or dist changes
    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/dist");

    // Capture git SHA for `rqb doctor` and `rqb --version` build info.
    // Falls back to "unknown" if git is unavailable (e.g. in CI without checkout depth).
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=RQB_BUILD_SHA={sha}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
