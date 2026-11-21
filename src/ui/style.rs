use egui_macroquad::egui;

/// Warm, rounded beach-shack theme (cream parchment panels, driftwood-brown
/// strokes, sea-glass teal accents) applied over egui's light visuals so
/// default widgets (labels, buttons, progress bars) pick it up without
/// every call site needing its own styling.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();

    let cream = egui::Color32::from_rgb(255, 247, 222);
    let wood = egui::Color32::from_rgb(214, 160, 90);
    let wood_dark = egui::Color32::from_rgb(120, 78, 38);
    let sea_teal = egui::Color32::from_rgb(38, 172, 170);
    let golden = egui::Color32::from_rgb(244, 178, 44);
    let text_brown = egui::Color32::from_rgb(74, 48, 24);

    visuals.window_fill = cream;
    visuals.panel_fill = cream;
    // Chunky framed panels (FR9): a thick driftwood border and a hard,
    // offset drop shadow, the classic adventure-game dialogue box look.
    visuals.window_stroke = egui::Stroke::new(4.0_f32, wood_dark);
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    visuals.window_shadow = egui::Shadow {
        offset: [5, 5],
        blur: 0,
        spread: 0,
        color: egui::Color32::from_rgba_unmultiplied(60, 36, 14, 150),
    };
    visuals.popup_shadow = visuals.window_shadow;
    visuals.override_text_color = Some(text_brown);
    // egui greys the title bar of whichever window is on top; in a HUD of
    // fixed panels "on top" means nothing, so keep every title the same.
    visuals.window_highlight_topmost = false;

    visuals.widgets.noninteractive.bg_fill = cream;
    visuals.widgets.noninteractive.weak_bg_fill = cream;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, text_brown);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = wood;
    visuals.widgets.inactive.weak_bg_fill = wood;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(2.0_f32, wood_dark);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(2.0_f32, wood_dark);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0_f32, wood_dark);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = golden;
    visuals.widgets.hovered.weak_bg_fill = golden;
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.active.bg_fill = sea_teal;
    visuals.widgets.active.weak_bg_fill = sea_teal;
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);

    visuals.selection.bg_fill = sea_teal;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, wood_dark);

    ctx.set_visuals(visuals);

    // Bigger, bolder type and roomier panels to match the frames.
    ctx.style_mut(|style| {
        use egui::{FontId, TextStyle};
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(21.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.5));
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    });
}
