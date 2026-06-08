pub trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32;
    fn clear(&mut self, id: u32);
}
