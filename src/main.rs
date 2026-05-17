#![windows_subsystem = "windows"]

use eframe::egui;
use eframe::egui::{FontData, FontDefinitions, FontFamily};
use env_manager::EnvManager;
use std::sync::Arc;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "开发环境变量管理器",
        options,
        Box::new(|cc| {
            // egui_extras::install_image_loaders(&cc.egui_ctx);
            setup_chinese_fonts(&cc.egui_ctx);
            Ok(Box::new(EnvManager::new(cc)))
        }),
    )
}

fn setup_chinese_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // Common CJK system font paths
    let candidates = &[
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyhbd.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\yahei.ttc",
    ];

    let mut found = false;
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            let name = "cjk_font".to_string();
            fonts
                .font_data
                .insert(name.clone(), Arc::from(FontData::from_owned(data)));
            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, name.clone());
            fonts
                .families
                .get_mut(&FontFamily::Monospace)
                .unwrap()
                .insert(0, name);
            found = true;
            break;
        }
    }

    if found {
        ctx.set_fonts(fonts);
    }
}
