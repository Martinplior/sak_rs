#[must_use]
pub fn info(message: impl Into<String>) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("信息")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
}

#[must_use]
pub fn warning(message: impl Into<String>) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("警告")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
}

#[must_use]
pub fn error(message: impl Into<String>) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("错误")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
}

#[must_use]
pub fn confirm(message: impl Into<String>) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("确认")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::OkCancel)
}
