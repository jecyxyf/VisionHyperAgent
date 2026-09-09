fn main() -> Result<(), Box<dyn std::error::Error>> {
    let configuration = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config("src/view/MainWindow.slint", configuration)?;
    Ok(())
}
