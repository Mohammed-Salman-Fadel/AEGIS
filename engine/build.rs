use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    // In dev mode (cargo build --features dev), skip frontend building entirely.
    // Developers should run `npm run dev` in frontend/ for hot-reload.
    #[cfg(feature = "dev")]
    {
        println!("cargo:warning=DEV MODE: Skipping frontend build. Use Vite dev server on port 5173 for hot-reload.");
        return;
    }

    let frontend_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("frontend");

    let dist_dir = frontend_dir.join("dist");

    // If dist/ already exists, skip rebuild (saves time during iterative dev).
    // To force a rebuild, delete dist/ or set env FORCE_FRONTEND_BUILD=1.
    let force = env::var("FORCE_FRONTEND_BUILD").is_ok();
    if !force && dist_dir.exists() {
        println!("cargo:warning=frontend/dist/ exists — skipping frontend build");
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed=FORCE_FRONTEND_BUILD");
        return;
    }

    // Check for npm
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };

    // Run npm ci first, then npm run build
    let install_status = Command::new(npm)
        .args(["ci"])
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to run npm ci");

    if !install_status.success() {
        panic!("Frontend dependency install failed (npm ci exited with {install_status})");
    }

    let build_status = Command::new(npm)
        .args(["run", "build:dist"])
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to run npm run build");

    if !build_status.success() {
        panic!("Frontend build failed (npm run build exited with {build_status})");
    }

    // Ensure dist/ exists
    assert!(
        dist_dir.exists(),
        "Frontend build completed but dist/ was not created"
    );

    // Tell cargo to rerun if any frontend source changes
    println!("cargo:rerun-if-changed=../frontend/src/");
    println!("cargo:rerun-if-changed=../frontend/package.json");
    println!("cargo:rerun-if-changed=../frontend/vite.config.ts");
    println!("cargo:rerun-if-changed=../frontend/tsconfig.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FORCE_FRONTEND_BUILD");
}
