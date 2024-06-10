use embassy_rp::{gpio, Peripheral};
use embassy_time::{Duration, Timer};

pub struct Blinker<'d> {
    pin: gpio::Output<'d>,
    duration: Duration,
}

impl<'d> Blinker<'d> {
    pub fn new(
        pin: impl Peripheral<P = impl gpio::Pin> + 'd,
        duration: Duration,
    ) -> Self {
        let pin = gpio::Output::new(pin, gpio::Level::Low);
        Self { pin, duration }
    }
    
    pub async fn blink(&mut self) {
        self.pin.set_high();
        Timer::after(self.duration).await;
        self.pin.set_low();
        Timer::after(self.duration).await;
    }
}
