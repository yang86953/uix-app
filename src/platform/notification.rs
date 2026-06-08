pub trait INotification {
    fn show(&mut self, title: &str, message: &str);
}
