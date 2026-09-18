use std::io::Cursor;

use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::browser;
use crate::http_server::ServerHandle;

const OPEN_MENU_ID: &str = "vha-open";
const EXIT_MENU_ID: &str = "vha-exit";
const TRAY_ICON_PNG: &[u8] = include_bytes!("../assets/tray-icon.png");

enum HostEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}

/// Runs the system-tray event loop and stops the server when the user exits.
pub fn run(url: &str, server: ServerHandle) -> Result<(), String> {
    log::info!("initializing system tray");
    let mut event_loop = EventLoopBuilder::<HostEvent>::with_user_event().build();

    let tray_proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = tray_proxy.send_event(HostEvent::Tray(event));
    }));

    let menu_proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(HostEvent::Menu(event));
    }));

    let (menu, open_item, exit_item) = build_menu()?;
    log::debug!("system tray menu created");
    let mut tray = None;
    let mut startup_error = None;

    let exit_code = event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => match build_tray(&menu, url) {
                Ok(icon) => {
                    tray = Some(icon);
                    log::info!("system tray icon created");

                    if let Err(error) = browser::open(url) {
                        log::error!("{error}");
                    }
                }
                Err(error) => {
                    log::error!("failed to create system tray icon: {error}");
                    startup_error = Some(error);
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::UserEvent(HostEvent::Tray(TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            })) => {
                if let Err(error) = browser::open(url) {
                    log::error!("{error}");
                }
            }
            Event::UserEvent(HostEvent::Menu(event)) => {
                if event.id == open_item.id() {
                    if let Err(error) = browser::open(url) {
                        eprintln!("{error}");
                    }
                } else if event.id == exit_item.id() {
                    log::info!("exit requested from system tray");
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });

    if let Some(error) = startup_error {
        log::error!("stopping HTTP server after tray startup failure: {error}");
        server.stop()?;
        return Err(error);
    }

    if exit_code != 0 {
        log::error!("system tray event loop exited with code {exit_code}");
        server.stop()?;
        return Err(format!("event loop exited with code {exit_code}"));
    }

    let result = server.stop();
    tray.take();
    if let Err(error) = &result {
        log::error!("failed to stop HTTP server: {error}");
    }

    result
}

fn build_menu() -> Result<(Menu, MenuItem, MenuItem), String> {
    let open_item = MenuItem::with_id(OPEN_MENU_ID, "打开界面", true, None);
    let exit_item = MenuItem::with_id(EXIT_MENU_ID, "退出", true, None);
    let menu = Menu::new();

    menu.append(&open_item)
        .and_then(|()| menu.append(&exit_item))
        .map_err(|error| {
            let message = format!("failed to build tray menu: {error}");
            log::error!("{message}");
            message
        })?;

    Ok((menu, open_item, exit_item))
}

fn build_tray(menu: &Menu, url: &str) -> Result<TrayIcon, String> {
    let icon = load_icon()?;
    log::debug!("system tray icon image decoded");

    TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_tooltip(format!("VisionHyperAgent · {url}"))
        .with_icon(icon)
        .build()
        .map_err(|error| format!("failed to create tray icon: {error}"))
}

fn load_icon() -> Result<tray_icon::Icon, String> {
    let mut decoder = png::Decoder::new(Cursor::new(TRAY_ICON_PNG));
    decoder.set_transformations(png::Transformations::normalize_to_color8());

    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("failed to read tray icon: {error}"))?;
    let mut pixels = vec![0_u8; reader.output_buffer_size()];
    let output_info = reader
        .next_frame(&mut pixels)
        .map_err(|error| format!("failed to decode tray icon: {error}"))?;

    if output_info.color_type != png::ColorType::Rgba {
        return Err("tray icon must contain RGBA pixels".to_string());
    }

    tray_icon::Icon::from_rgba(pixels, output_info.width, output_info.height)
        .map_err(|error| format!("failed to create tray icon image: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tray_icon_loads() {
        assert!(load_icon().is_ok());
    }

    #[test]
    fn embedded_tray_icon_is_square_rgba_with_transparent_margins() {
        let mut reader = png::Decoder::new(Cursor::new(TRAY_ICON_PNG))
            .read_info()
            .expect("embedded PNG must be valid");
        let mut pixels = vec![0_u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).expect("PNG must decode");

        assert_eq!((info.width, info.height), (64, 64));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);

        let width = info.width as usize;
        let height = info.height as usize;
        let alpha = |x: usize, y: usize| pixels[(y * width + x) * 4 + 3];
        for x in 0..width {
            assert_eq!(alpha(x, 0), 0);
            assert_eq!(alpha(x, height - 1), 0);
        }
        for y in 0..height {
            assert_eq!(alpha(0, y), 0);
            assert_eq!(alpha(width - 1, y), 0);
        }
        assert_eq!(alpha(width / 2, height / 2), 255);
    }
}
