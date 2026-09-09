use slint::platform::{
    Platform, WindowAdapter,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};
use slint::{ComponentHandle, PhysicalSize, Rgb8Pixel};
use std::rc::Rc;
use vision_hyper_agent::view::{MainWindow, ViewPage};

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn native_ui_constructs_renders_and_preserves_drafts() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(adapter.clone())))?;
    let window = MainWindow::new()?;
    assert_eq!(window.get_current_page(), ViewPage::Inference);
    window.set_chat_draft("切页保留草稿".into());
    window.set_current_page(ViewPage::Models);
    window.set_models_expanded(false);
    assert_eq!(window.get_chat_draft(), "切页保留草稿");
    window.set_current_page(ViewPage::Inference);
    window.window().set_size(PhysicalSize::new(1440, 900));
    window.show()?;
    window.window().request_redraw();
    let mut pixels = vec![Rgb8Pixel::default(); 1440 * 900];
    assert!(adapter.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 1440);
    }));
    assert!(
        pixels.iter().any(|pixel| *pixel != pixels[0]),
        "主窗口不应渲染为空白"
    );
    window.hide()?;
    Ok(())
}
