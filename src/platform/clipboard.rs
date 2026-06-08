pub trait IClipboard {
    fn text(&self) -> String;
    fn set_text(&mut self, text: &str);
    fn has_text(&self) -> bool;
}
