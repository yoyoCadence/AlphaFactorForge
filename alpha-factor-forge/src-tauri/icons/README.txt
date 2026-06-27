AlphaFactorForge app icons go here.

Generate them from a single source logo with the Tauri CLI:

    cargo tauri icon path/to/logo.png

This produces icon.png + platform-specific sizes that tauri.conf.json expects.
Without these, `cargo tauri dev` / `build` will fail on the bundle step.

(Placeholder file — replace this folder's contents with generated icons.)
