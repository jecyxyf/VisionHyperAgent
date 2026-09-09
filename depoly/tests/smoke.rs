use slint::platform::{
    Platform, PointerEventButton, WindowAdapter, WindowEvent,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};
use slint::{ComponentHandle, LogicalPosition, ModelRc, PhysicalSize, Rgb8Pixel, VecModel};
use std::rc::Rc;
use vision_hyper_agent::view::{MainWindow, ModelEntry, ViewPage, bind_main_window};
use vision_hyper_agent::view_model::{MainPage, MainWindowViewModel};

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn render(window: &MainWindow, adapter: &MinimalSoftwareWindow) {
    let size = window.window().size();
    window.window().request_redraw();
    let mut pixels = vec![Rgb8Pixel::default(); (size.width * size.height) as usize];
    assert!(adapter.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, size.width as usize);
    }));
    assert!(
        pixels.iter().any(|pixel| *pixel != pixels[0]),
        "主窗口不应渲染为空白"
    );
}

fn drag(window: &MainWindow, adapter: &MinimalSoftwareWindow, from: (f32, f32), to: (f32, f32)) {
    let start = LogicalPosition::new(from.0, from.1);
    let end = LogicalPosition::new(to.0, to.1);
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position: start });
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position: end });
    render(window, adapter);
    // Moving to the same absolute point again must not accumulate the handle's own movement.
    let first = window.get_pane_layout();
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position: end });
    render(window, adapter);
    assert_eq!(window.get_pane_layout(), first, "拖拽不得累积位移或抖动");
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position: end,
            button: PointerEventButton::Left,
        });
}

fn close_to(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1.0,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn native_ui_constructs_renders_and_preserves_drafts() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(adapter.clone())))?;
    let window = MainWindow::new()?;
    let view_model = Rc::new(MainWindowViewModel::new());
    bind_main_window(&window, Rc::clone(&view_model));
    assert_eq!(window.get_current_page(), ViewPage::Inference);
    window.set_chat_draft("切页保留草稿".into());
    window.invoke_page_requested(ViewPage::Models);
    assert_eq!(view_model.current_page(), MainPage::Models);
    assert_eq!(window.get_current_page(), ViewPage::Models);
    window.set_models_expanded(false);
    assert_eq!(window.get_chat_draft(), "切页保留草稿");
    window.invoke_page_requested(ViewPage::Inference);
    assert_eq!(view_model.current_page(), MainPage::Inference);
    window.window().set_size(PhysicalSize::new(1440, 900));
    window.show()?;
    render(&window, &adapter);
    let defaults = window.get_pane_layout();

    drag(&window, &adapter, (400.0, 722.0), (400.0, 642.0));
    close_to(window.get_pane_layout().inference_records_height, 218.0);
    window.set_current_page(ViewPage::Settings);
    window.set_current_page(ViewPage::Inference);
    close_to(window.get_pane_layout().inference_records_height, 218.0);
    render(&window, &adapter);
    drag(&window, &adapter, (400.0, 642.0), (400.0, 10000.0));
    close_to(window.get_pane_layout().inference_records_height, 120.0);

    window.set_pane_layout(defaults.clone());
    render(&window, &adapter);
    drag(&window, &adapter, (164.0, 400.0), (224.0, 400.0));
    close_to(window.get_pane_layout().navigation_width, 200.0);
    drag(&window, &adapter, (224.0, 400.0), (-10000.0, 400.0));
    close_to(window.get_pane_layout().navigation_width, 140.0);
    window.window().dispatch_event(WindowEvent::KeyPressed {
        text: "\u{f703}".into(),
    });
    window.window().dispatch_event(WindowEvent::KeyReleased {
        text: "\u{f703}".into(),
    });
    close_to(window.get_pane_layout().navigation_width, 156.0);
    window.set_pane_layout(defaults.clone());
    render(&window, &adapter);
    drag(&window, &adapter, (1004.0, 400.0), (944.0, 400.0));
    close_to(window.get_pane_layout().agent_width, 472.0);

    window.set_pane_layout(defaults.clone());
    render(&window, &adapter);
    drag(&window, &adapter, (1200.0, 726.0), (1200.0, 686.0));
    close_to(window.get_pane_layout().chat_composer_height, 172.0);

    window.set_pane_layout(defaults.clone());
    window.set_current_page(ViewPage::Preannotation);
    render(&window, &adapter);
    drag(&window, &adapter, (496.0, 450.0), (536.0, 450.0));
    close_to(window.get_pane_layout().preannotation_editor_width, 340.0);
    drag(&window, &adapter, (700.0, 527.0), (700.0, 567.0));
    assert!(window.get_pane_layout().preannotation_analysis_ratio > 0.55);

    window.set_pane_layout(defaults.clone());
    window.set_current_page(ViewPage::Annotation);
    window.set_annotation_grid(false);
    render(&window, &adapter);
    drag(&window, &adapter, (312.0, 450.0), (352.0, 450.0));
    close_to(window.get_pane_layout().annotation_list_width, 156.0);

    window.set_pane_layout(defaults.clone());
    window.set_current_page(ViewPage::Models);
    window.set_model_entries(ModelRc::new(VecModel::from(vec![ModelEntry {
        name: "UI test fixture".into(),
        format: "ONNX".into(),
        path: "/ui-fixture/model.onnx".into(),
    }])));
    window.set_selected_model(0);
    render(&window, &adapter);
    drag(&window, &adapter, (400.0, 748.0), (400.0, 708.0));
    close_to(window.get_pane_layout().models_details_height, 152.0);
    window.set_selected_model(-1);
    render(&window, &adapter);

    window.set_pane_layout(defaults.clone());
    window.set_current_page(ViewPage::Training);
    render(&window, &adapter);
    drag(&window, &adapter, (406.0, 400.0), (446.0, 400.0));
    close_to(window.get_pane_layout().training_parameters_width, 250.0);
    drag(&window, &adapter, (700.0, 746.0), (700.0, 706.0));
    close_to(window.get_pane_layout().training_log_height, 154.0);
    window.set_pane_layout(defaults.clone());
    render(&window, &adapter);
    // The chart header uses font metrics; search only the narrow expected divider band.
    let mut found = false;
    for y in (460..=500).step_by(4) {
        drag(
            &window,
            &adapter,
            (700.0, y as f32),
            (700.0, y as f32 + 20.0),
        );
        if (window.get_pane_layout().training_loss_ratio - 0.5).abs() > 0.01 {
            found = true;
            break;
        }
    }
    assert!(found, "两块训练曲线之间应有可拖拽分隔条");

    let preserved = window.get_pane_layout();
    for size in [PhysicalSize::new(1120, 720), PhysicalSize::new(1920, 1080)] {
        window.window().set_size(size);
        for page in [
            ViewPage::Inference,
            ViewPage::Models,
            ViewPage::Preannotation,
            ViewPage::Annotation,
            ViewPage::Pretraining,
            ViewPage::Training,
            ViewPage::Settings,
            ViewPage::About,
        ] {
            window.set_current_page(page);
            render(&window, &adapter);
        }
        assert_eq!(
            window.get_pane_layout(),
            preserved,
            "缩放和切页不得覆盖用户尺寸偏好"
        );
        assert_eq!(window.get_chat_draft(), "切页保留草稿");
    }
    window.hide()?;
    Ok(())
}
