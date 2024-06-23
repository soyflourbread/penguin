use embassy_rp::{pio, gpio};

use defmt::{info};
use embassy_time::Timer;

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

mod api {
    #[derive(Debug)]
    pub enum CommandError {
        Invalid,
    }

    pub enum Command {
        MotorStop,
        Beep { count: u8 },
        EscInfo,
        Reverse(bool),
        Led { id: u8, enabled: bool },
        Throttle(u16),
    }

    impl TryFrom<Command> for u16 {
        type Error = CommandError;

        fn try_from(op_0: Command) -> Result<Self, Self::Error> {
            let ret = match op_0 {
                Command::MotorStop => { Self::MIN }
                Command::Beep { count } => {
                    if count < 1 || count > 5 {
                        return Err(Self::Error::Invalid);
                    }
                    count as u16
                }
                Command::EscInfo => { 6 }
                Command::Reverse(rev) => {
                    if rev { 21 } else { 20 }
                }
                Command::Led { id, enabled } => {
                    if id > 3 { return Err(Self::Error::Invalid); }
                    let mut ret = if enabled { 22 } else { 26 };
                    ret += id as u16;
                    ret
                }
                Command::Throttle(throttle) => {
                    if throttle > 1999 {
                        return Err(Self::Error::Invalid);
                    }
                    throttle + 48
                }
            };
            Ok(ret)
        }
    }

    pub struct Frame {
        command: u16,
        telemetry: bool,
    }

    impl Frame {
        pub fn new(command: u16, telemetry: bool) -> Self {
            Self { command, telemetry }
        }

        fn checksum(&self) -> u16 {
            let frame = (self.command << 1) | (self.telemetry as u16);
            let mut ret = frame;
            ret ^= frame >> 4;
            ret ^= frame >> 8;
            ret &= 0x0F;
            ret
        }
    }

    impl From<Frame> for u16 {
        fn from(op_0: Frame) -> Self {
            let mut ret = u16::MIN;
            ret |= op_0.command << 5;
            ret |= (op_0.telemetry as u16) << 4;
            ret |= op_0.checksum();
            ret
        }
    }
}

pub struct PioDshot<'a, P: pio::Instance, const SM: usize> {
    sm: pio::StateMachine<'a, P, SM>,
}

impl<'a, P: pio::Instance, const SM: usize> PioDshot<'a, P, SM> {
    pub fn new(
        common: &mut pio::Common<'a, P>,
        mut sm: pio::StateMachine<'a, P, SM>,
        pin: impl pio::PioPin,
    ) -> Self {
        // 6:2 for high, 3:5 for low
        let prg = pio_proc::pio_asm!(
            r#"
            .side_set 1 opt

            fetch_frame:
                out null 16
            
            loop_entry:
                set x 15 ; will be executed 16 times
            loop_start:
                out y 1 side 1 [1]
                jmp !y falling_edge ; 3 cycles of high so far
                nop [2] ; 3 additional cycles of high
            falling_edge:
                jmp y-- loop_end side 0 ; 1 cycle of low so far
                nop [2] ; 3 additional cycles of low
            loop_end:
                jmp x-- loop_start ; 1 extra cycle of low
            
            reset_entry: ; delay 128 cycles (why)
                set x 14 [7] ; delay 8 cycles
            reset_start:
            reset_end:
                jmp x-- reset_start [7] ; delay 8 cycles
            "#
        );
        let pin = common.make_pio_pin(pin);
        sm.set_pins(gpio::Level::Low, &[&pin]);
        sm.set_pin_dirs(pio::Direction::Out, &[&pin]);

        let mut cfg = pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.use_program(&common.load_program(&prg.program), &[&pin]);
        cfg.shift_out = pio::ShiftConfig {
            auto_fill: true,
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8;
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();
        sm.set_config(&cfg);
        sm.set_enable(true);

        Self { sm }
    }
    
    pub async fn arm(&mut self) {
        for _i in 0..2000 {
            self.motor_stop().await;
            Timer::after_millis(1).await;
        }
        for _i in 0..1000 {
            self.direction(false).await;
            Timer::after_millis(1).await;
        }
    }
    
    pub async fn motor_stop(&mut self) {
        let frame = api::Frame::new(
            api::Command::MotorStop.try_into().unwrap(),
            false,
        );
        let frame: u16 = frame.into();
        self.sm.tx().wait_push(frame as u32).await;
    }

    pub async fn beep(&mut self) {
        let frame = api::Frame::new(
            api::Command::Beep{ count: 1 }.try_into().unwrap(),
            true,
        );
        let frame: u16 = frame.into();
        self.sm.tx().wait_push(frame as u32).await;
    }

    pub async fn throttle(&mut self, throttle: u16) {
        let throttle = throttle.min(1999);
        let frame = api::Frame::new(
            api::Command::Throttle(throttle).try_into().unwrap(),
            false,
        );
        let frame: u16 = frame.into();
        self.sm.tx().wait_push(frame as u32).await;
    }
    
    pub async fn direction(&mut self, reverse: bool) {
        let frame = api::Frame::new(
            api::Command::Reverse(reverse).try_into().unwrap(),
            true,
        );
        let frame: u16 = frame.into();
        self.sm.tx().wait_push(frame as u32).await;
    }
}
