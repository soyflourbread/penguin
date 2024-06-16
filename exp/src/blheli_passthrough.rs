use embassy_rp::{pio, gpio};

use defmt::{info};
use fixed::FixedU32;
use fixed::types::extra::U8;
use heapless::String;
use core::fmt::Write;

mod api {
    #[repr(u8)]
    pub enum Command {
        Restart = 0,

        KeepAlive = 0xFD,
        SetBuffer = 0xFE,
        SetAddress = 0xFF,
    }

    pub struct CRC { value: u16 }

    impl CRC {
        pub fn next(&mut self, mut data: u8) {
            for _ in 0..8 {
                let mask = (data & 0x01) as u16 ^ (self.value & 0x0001);
                self.value >>= 1;
                if mask > 0 { self.value ^= 0xA001; }
                data >>= 1;
            }
        }

        pub fn drain(self) -> u16 {
            self.value
        }
    }
}

pub struct BlHeliPassThrough<'a, P: pio::Instance, const SM: usize> {
    sm: pio::StateMachine<'a, P, SM>,

    cfg_rx: pio::Config<'a, P>,
    cfg_tx: pio::Config<'a, P>,
}

impl<'a, P: pio::Instance, const SM: usize> BlHeliPassThrough<'a, P, SM> {
    pub fn new(
        common: &mut pio::Common<'a, P>,
        mut sm: pio::StateMachine<'a, P, SM>,
        pin: impl pio::PioPin,
    ) -> Self {
        let clk_div = (429u32 << 8) | 192;
        let clk_div: FixedU32<U8> = FixedU32::from_bits(clk_div);
        let mut frame: String<128> = String::new();
        write!(frame, "{}", clk_div).unwrap();
        info!("clk_div: {=str}", frame);

        let prg_rx = pio_proc::pio_asm!(
            r#"
            set pindirs, 0 [5]
            discard:
                mov isr, null
            .wrap_target
            set x, 7
            wait 0 pin, 0 [23] ; wait for start bit, delay 1.5 bits to sample in the center of each bit
            read_bit:
                in pins, 1 [14] ; sample 8 bits
                jmp x--, read_bit
                jmp pin, push_byte ; discard bit if the stop bit is not present
                jmp discard [2] ; reduced delay to leave room for slight clock deviations
            push_byte:
                push block [3] ; reduced delay to leave room for slight clock deviations
            .wrap
            "#
        );
        let prg_tx = pio_proc::pio_asm!(
            r#"
            set pins, 1 [2]
            set pindirs, 1 [2]
            .wrap_target
            pull block
            set pins, 0 [15] ; start bit
            write_bit:
                out pins, 1 [14] ; 8 data bits
                jmp !osre, write_bit
                set pins, 1 [14] ; stop bit
            .wrap
            "#
        );

        let pin = common.make_pio_pin(pin);

        let mut cfg_rx = pio::Config::default();
        cfg_rx.set_set_pins(&[&pin]);
        cfg_rx.set_in_pins(&[&pin]);
        cfg_rx.set_jmp_pin(&pin);
        cfg_rx.use_program(&common.load_program(&prg_rx.program), &[]);
        cfg_rx.shift_in = pio::ShiftConfig {
            // shift right, no autofill, threshold 32
            threshold: 32,
            ..Default::default()
        };
        cfg_rx.clock_divider = clk_div;

        let mut cfg_tx = pio::Config::default();
        cfg_tx.set_set_pins(&[&pin]);
        cfg_tx.set_out_pins(&[&pin]);
        cfg_tx.use_program(&common.load_program(&prg_tx.program), &[]);
        cfg_rx.shift_out = pio::ShiftConfig {
            // shift right, no autofill, threshold 8
            threshold: 8,
            ..Default::default()
        };
        cfg_tx.clock_divider = clk_div;
        sm.set_pin_dirs(pio::Direction::In, &[&pin]);
        sm.set_config(&cfg_rx);
        sm.set_enable(true);

        Self { sm, cfg_rx, cfg_tx }
    }

    pub async fn read_u8(&mut self) -> u8 {
        self.sm.rx().wait_pull().await as u8
    }

    pub async fn transmit(&mut self, buf: &[u8]) {
        info!("enabling tx mode");
        self.sm.set_enable(false);
        self.sm.set_config(&self.cfg_tx);
        self.sm.set_enable(true);

        for &data in buf {
            self.sm.tx().wait_push(data as u32).await;
        }

        while !self.sm.tx().empty() {}
        while !self.sm.tx().stalled() {}

        info!("disabling tx mode");
        self.sm.set_enable(false);
        self.sm.set_config(&self.cfg_rx);
        self.sm.set_enable(true);
    }
}
