use embassy_rp::gpio;
use embassy_time::{Duration, Timer};

pub struct Button<'a> {
    input: gpio::Input<'a>,
    debounce: Duration,
}

impl<'a> Button<'a> {
    pub fn new(input: gpio::Input<'a>, debounce: Duration) -> Self {
        Self { input, debounce }
    }

    pub async fn debounce(&mut self) -> gpio::Level {
        loop {
            self.input.wait_for_any_edge().await;
            Timer::after(self.debounce).await;
            return self.input.get_level();
        }
    }
}
