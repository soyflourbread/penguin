use embassy_rp::{Peripheral, pwm};
use embassy_rp::pwm::{ChannelAPin, ChannelBPin, Slice};

pub struct ServoAB<'d> {
    pwm: pwm::Pwm<'d>,
    config: pwm::Config,

    period: f32,
    cmp_min: u16,
    cmp_max: u16,
}

impl<'d> ServoAB<'d> {
    pub fn new<T: Slice>(
        pwm_slice: impl Peripheral<P = T> + 'd,
        pin_a: impl Peripheral<P = impl ChannelAPin<T>> + 'd,
        pin_b: impl Peripheral<P = impl ChannelBPin<T>> + 'd,
        period: f32, ts_min: f32, ts_max: f32,
        pos_init: f32,
    ) -> Self {
        let divider = 0x20u8;

        let mut top = embassy_rp::clocks::pll_sys_freq() as f32;
        top *= period;
        top /= 1000.0;
        top /= 2.0;
        top /= divider as f32;
        top -= 1.0;
        assert!(top < u16::MAX as f32);
        let top = top as u16;

        let cmp_min = (ts_min * top as f32 / period) as u16;
        let cmp_max = (ts_max * top as f32 / period) as u16;
        let cmp_init = Self::pos_to_cmp(cmp_min, cmp_max, pos_init);

        let mut config: pwm::Config = Default::default();
        config.phase_correct = true;
        config.divider = divider.into();
        config.top = top;
        config.compare_a = cmp_init;
        config.compare_b = cmp_init;

        let pwm = pwm::Pwm::new_output_ab(pwm_slice, pin_a, pin_b, config.clone());

        Self {
            pwm, config,
            period, cmp_min, cmp_max,
        }
    }

    fn pos_to_cmp(cmp_min: u16, cmp_max: u16, pos: f32) -> u16 {
        let mut ret = (cmp_max - cmp_min) as f32;
        ret *= pos;
        ret += cmp_min as f32;
        ret as u16
    }

    pub fn set_position_a(&mut self, pos: f32) {
        self.config.compare_a = Self::pos_to_cmp(self.cmp_min, self.cmp_max, pos);
        self.pwm.set_config(&self.config);
    }

    pub fn set_position_b(&mut self, pos: f32) {
        self.config.compare_a = Self::pos_to_cmp(self.cmp_min, self.cmp_max, pos);
        self.pwm.set_config(&self.config);
    }
}
