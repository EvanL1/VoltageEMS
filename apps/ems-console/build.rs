fn main() {
    // Build script - unwrap is acceptable (build failures halt compilation anyway)
    #[allow(clippy::disallowed_methods)]
    {
        slint_build::compile("ui/main.slint").unwrap();
    }
}
