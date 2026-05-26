/// Ensure `web/dist/` exists so rust-embed compiles even before `npm run build`.
/// The real build artefacts are written by `cd web && npm run build`.
fn main() {
    let dist = std::path::Path::new("web/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).expect("cannot create web/dist");
        std::fs::write(
            dist.join("index.html"),
            "<!DOCTYPE html><html><body>\
             <h1>Trellis UI not built</h1>\
             <p>Run <code>cd web &amp;&amp; npm run build</code> then restart <code>trellis serve</code>.</p>\
             </body></html>",
        )
        .expect("cannot write placeholder index.html");
    }
    // Re-run if the web source or dist changes
    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/dist");
}
