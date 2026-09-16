//! System tray icon.
//! Left-click: open browser. Right-click menu: open / exit.

use std::sync::mpsc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub enum TrayEvent {
    OpenBrowser,
    Exit,
}

pub struct TrayHandle {
    _tray: TrayIcon,
}

pub fn create_tray(url: &str) -> Result<(TrayHandle, mpsc::Receiver<TrayEvent>), String> {
    let (tx, rx) = mpsc::channel();

    let open_item = MenuItem::with_id("open", "打开界面", true, None);
    let exit_item = MenuItem::with_id("exit", "退出", true, None);

    let menu = Menu::new();
    menu.append(&open_item).map_err(|e| e.to_string())?;
    menu.append(&exit_item).map_err(|e| e.to_string())?;

    let open_id = open_item.id().clone();
    let exit_id = exit_item.id().clone();
    let url_owned = url.to_string();

    // Global menu event handler
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == open_id {
            let _ = webbrowser::open(&url_owned);
            let _ = tx.send(TrayEvent::OpenBrowser);
        } else if event.id == exit_id {
            let _ = tx.send(TrayEvent::Exit);
        }
    }));

    let tooltip = format!("VisionHyperAgent - {}", url);
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(&tooltip)
        .build()
        .map_err(|e| format!("tray build failed: {}", e))?;

    Ok((TrayHandle { _tray: tray }, rx))
}
