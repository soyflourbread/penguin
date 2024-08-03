use defmt::info;
use embassy_rp::{dma, gpio, pio};
use embassy_time::{Duration, Ticker, Timer};

use crate::{api, DshotTx};
use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

pub struct PioDshot<'a, P: pio::Instance, const SM: usize> {
    sm: pio::StateMachine<'a, P, SM>,
    pin: pio::Pin<'a, P>,
    origin: u8,
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
            write_entry:
                pull noblock
                mov x, osr
                out null 16 ; 3 cycles total
            write_start:
                set pins 0 [14] ; 15 cycles of low
                out pins 1 [14] ; 15 cycles of out
                set pins 1 [8] ; 9 cycle of high
            write_end:
                jmp !osre write_start ; 1 extra cycle of high

            switch_entry:
                push noblock
                set pindirs 0
                set y 8 [29]; give time to switch lines, 10 * 32 = 320 cycles
            switch_start:
            switch_end:
                jmp y-- switch_start [31]

            ; stall for at most 360 cycles = 30 us, 40 cycles left
            wait_entry:
                set y 31 ; timing not strict
            wait_start:
                jmp !y cleanup
                jmp y-- wait_end
            wait_end:
                jmp pin wait_start

            read_entry: ; 32 cycles per bit
                set y 19 ; executed 20 times
                in null 16 [28] ; first bit is always 0
            read_start:
                nop [3] ; wait 4 cycles before starting
                in pins 1 [7] ; get first bit
                in pins 1 [7] ; get second bit
                in pins 1 [7] ; get third bit
                in pins 1 [1]; get last bit
                push iffull noblock
            read_end:
                jmp y-- read_start

            cleanup:
                set pindirs 1 [31]; restore pin to out
                set pins 1 [31]; push final bit and reset pin level

            ; 640 cycles for a frame, 637 cycles required
            idle_entry:
                set y, 18 [28]; execute 19 times, 29 cycles
            idle_start:
            idle_end:
                jmp y-- idle_start [31] ; 32 cycles x 19 = 608
            "#
        );
        let prg = common.load_program(&prg.program);

        let mut pin = common.make_pio_pin(pin);
        pin.set_pull(gpio::Pull::Up);
        sm.set_pin_dirs(pio::Direction::Out, &[&pin]);
        sm.set_pins(gpio::Level::High, &[&pin]);

        let mut cfg = pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.set_in_pins(&[&pin]);
        cfg.set_out_pins(&[&pin]);
        cfg.set_jmp_pin(&pin);
        cfg.use_program(&prg, &[]);
        cfg.shift_out = pio::ShiftConfig {
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        cfg.shift_in = pio::ShiftConfig {
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8 * 5; // 40 cycles for dshot frame bit, 32 cycles for EDT frames bit
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();

        let origin = prg.origin;

        sm.set_config(&cfg);
        let mut ret = Self { sm, pin, origin };
        ret.send_command(crate::api::Command::ExtendedTelemetry { enabled: true });
        ret
    }
}

impl<'a, P: pio::Instance, const SM: usize> DshotTx for PioDshot<'a, P, SM> {
    type Output = Option<[u32; 4]>;

    fn entry(&mut self) {
        self.sm.set_enable(true);
    }

    fn send_frame(&mut self, frame: u16) {
        self.sm.tx().push(!frame as u32);
    }

    fn send_command(&mut self, command: api::Command) {
        let command = command.try_into().unwrap();
        let frame = api::FrameBuilder::new(api::Frame {
            command,
            telemetry: true,
        })
        .invert()
        .build();
        self.send_frame(frame)
    }

    fn drain(&mut self) -> Self::Output {
        if !self.sm.rx().full() {
            return None;
        }
        let frame = [
            self.sm.rx().try_pull()?,
            self.sm.rx().try_pull()?,
            self.sm.rx().try_pull()?,
            self.sm.rx().try_pull()?,
        ];
        Some(frame)
    }
}
