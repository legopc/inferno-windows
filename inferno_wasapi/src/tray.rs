//! System tray icon with status menu.

pub fn run_tray() {
    use tray_icon::{
        menu::{Menu, MenuItem, PredefinedMenuItem},
        TrayIconBuilder,
    };
    
    let menu = Menu::new();
    let status_item = MenuItem::new("InfernoAoIP — Running", false, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append_items(&[&status_item, &separator, &quit_item]).ok();
    
    // Load a simple icon (embedded 16x16 ICO)
    let icon = tray_icon::Icon::from_rgba(
        vec![0u8, 128u8, 255u8, 255u8].repeat(16 * 16),
        16, 16,
    ).expect("icon creation failed");
    
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("InfernoAoIP Dante Virtual Soundcard")
        .with_icon(icon)
        .build()
        .expect("tray icon build failed");
    
    tracing::info!("System tray icon active");
    
    // Event loop — handle tray menu clicks
    let menu_channel = tray_icon::menu::MenuEvent::receiver();
    let quit_id = quit_item.id().clone();
    
    loop {
        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_id {
                tracing::info!("Tray: quit requested");
                std::process::exit(0);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
