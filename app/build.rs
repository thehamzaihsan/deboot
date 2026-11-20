fn main() {
    slint_build::compile("ui/app.slint").unwrap();

    #[cfg(target_os = "windows")]
    {
        use embed_manifest::manifest::ExecutionLevel;
        use embed_manifest::{embed_manifest, new_manifest};

        embed_manifest(
            new_manifest("deboot.app").requested_execution_level(ExecutionLevel::RequireAdministrator),
        )
        .expect("failed to embed Windows manifest requesting administrator privileges");
    }
}
