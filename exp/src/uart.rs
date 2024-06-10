use core::convert::Infallible;

use embassy_rp::{pio, gpio};
use embedded_io_async::{ErrorType, Write};

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

pub struct PioUartTx<'a, P: pio::Instance, const SM: usize> {
    sm_tx: pio::StateMachine<'a, P, SM>,
}

impl<'a, P: pio::Instance, const SM: usize> PioUartTx<'a, P, SM> {
    pub fn new(
        common: &mut pio::Common<'a, P>,
        mut sm_tx: pio::StateMachine<'a, P, SM>,
        tx_pin: impl pio::PioPin,
        baud: u64,
    ) -> Self {
        let prg = pio_proc::pio_asm!(
                r#"
                .side_set 1 opt

                ; An 8n1 UART transmit program.
                ; OUT pin 0 and side-set pin 0 are both mapped to UART TX pin.

                    pull       side 1 [7]  ; Assert stop bit, or stall with line in idle state
                    set x, 7   side 0 [7]  ; Preload bit counter, assert start bit for 8 clocks
                bitloop:                   ; This loop will run 8 times (8n1 UART)
                    out pins, 1            ; Shift 1 bit from OSR to the first OUT pin
                    jmp x-- bitloop   [6]  ; Each loop iteration is 8 cycles.
            "#
            );
        let tx_pin = common.make_pio_pin(tx_pin);
        sm_tx.set_pins(gpio::Level::High, &[&tx_pin]);
        sm_tx.set_pin_dirs(pio::Direction::Out, &[&tx_pin]);

        let mut cfg = pio::Config::default();

        cfg.set_out_pins(&[&tx_pin]);
        cfg.use_program(&common.load_program(&prg.program), &[&tx_pin]);
        cfg.shift_out.auto_fill = false;
        cfg.shift_out.direction = pio::ShiftDirection::Right;
        cfg.fifo_join = pio::FifoJoin::TxOnly;
        cfg.clock_divider = (U56F8!(125_000_000) / (8 * baud)).to_fixed();
        sm_tx.set_config(&cfg);
        sm_tx.set_enable(true);

        Self { sm_tx }
    }

    pub async fn write_u8(&mut self, data: u8) {
        self.sm_tx.tx().wait_push(data as u32).await;
    }
}

impl<P: pio::Instance, const SM: usize> ErrorType for PioUartTx<'_, P, SM> {
    type Error = Infallible;
}

impl<P: pio::Instance, const SM: usize> Write for PioUartTx<'_, P, SM> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        for byte in buf {
            self.write_u8(*byte).await;
        }
        Ok(buf.len())
    }
}
