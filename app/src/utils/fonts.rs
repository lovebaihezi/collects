use egui::{
    FontData, FontFamily,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};

pub fn add_font(ctx: &egui::Context, font_data: Vec<u8>) {
    let data = FontData::from_owned(font_data);
    ctx.add_font(FontInsert::new(
        "source han serif",
        data,
        vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Lowest,
        }],
    ));
}
