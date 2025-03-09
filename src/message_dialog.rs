#[must_use]
pub fn info(message: &str) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_title("信息")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
}

#[must_use]
pub fn warning(message: &str) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_title("警告")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
}

#[must_use]
pub fn error(message: &str) -> rfd::MessageDialog {
    rfd::MessageDialog::new()
        .set_title("错误")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
}
