use glow::HasContext;

// Mesa can retain a compiler worker per CPU on large machines. A launcher has
// few shaders, so ask supporting drivers to use two workers for this context.
// This is a GL hint, scoped to rayslash; it does not change driver environment
// variables or affect other applications.
pub(crate) fn limit_shader_compiler_threads(api: &slint::GraphicsAPI<'_>) {
    let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
        return;
    };
    // Slint invokes RenderingSetup on the render thread with the GL context
    // current, and its loader returns this context's entry points.
    let context = unsafe { glow::Context::from_loader_function_cstr(get_proc_address) };
    if context
        .supported_extensions()
        .contains("GL_KHR_parallel_shader_compile")
        || context
            .supported_extensions()
            .contains("GL_ARB_parallel_shader_compile")
    {
        // The extension is advertised by the current context; this changes
        // only its compiler concurrency hint and preserves normal rendering.
        unsafe { context.max_shader_compiler_threads(2) };
    }
}
