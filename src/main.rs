mod ui;
mod fs_ops;
mod clipboard_ops;
mod file_reader;

fn main() -> iced::Result {
    ui::start_ui()
}
