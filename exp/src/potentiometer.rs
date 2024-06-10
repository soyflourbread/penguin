use embassy_rp::{Peripheral, adc, gpio};
use embassy_rp::adc::AdcPin;

pub struct Potentiometer<'a> {
    channel: adc::Channel<'a>,
}

impl<'a> Potentiometer<'a> {
    pub fn new(
        s: impl Peripheral<P = impl AdcPin> + 'a
    ) -> Self {
        let channel = adc::Channel::new_pin(s, gpio::Pull::None);
        Self { channel }
    }

    pub async fn voltage(&mut self, adc: &mut adc::Adc<'_, adc::Async>) -> Result<f32, adc::Error> {
        let raw_vol = adc.read(&mut self.channel).await?;

        let mut ret = raw_vol as f32;
        ret *= 3.23;
        ret *= 3.0;
        ret /= 4096.0;
        Ok(ret)
    }
}
