use embassy_rp::{Peripheral, peripherals, adc};

pub struct Thermometer<'a> {
    channel: adc::Channel<'a>,
}

impl<'a> Thermometer<'a> {
    pub fn new(
        s: impl Peripheral<P = peripherals::ADC_TEMP_SENSOR> + 'a
    ) -> Self {
        let channel = adc::Channel::new_temp_sensor(s);
        Self { channel }
    }

    pub async fn temperature(&mut self, adc: &mut adc::Adc<'_, adc::Async>) -> Result<f32, adc::Error> {
        let raw_temp = adc.read(&mut self.channel).await?;

        // See RP2040 datasheet, chapter 4.9.5. Temperature Sensor
        let temp = 27.0 - (raw_temp as f32 * 3.3 / 4096.0 - 0.706) / 0.001721;
        let sign = if temp < 0.0 { -1.0 } else { 1.0 };
        let rounded_temp_x10: i16 = ((temp * 10.0) + 0.5 * sign) as i16;
        let ret = (rounded_temp_x10 as f32) / 10.0;
        Ok(ret)
    }
}
