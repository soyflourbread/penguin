#![no_std]
#![no_main]

use core::fmt::Write;
use heapless::String;

use embassy_executor::Spawner;
use embassy_rp::{adc, bind_interrupts, gpio, pwm};
use embassy_rp::{pio, i2c, peripherals, uart};
use embassy_time::{Delay, Duration, Ticker, Timer};

use defmt::{info};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

#[embassy_executor::task]
async fn motor() {

}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let pio::Pio {
        mut common, sm0, sm1, ..
    } = pio::Pio::new(p.PIO0, Irqs);
    let mut uart_0 = penguin_exp::uart::PioUartTx::new(
        &mut common, sm0,
        p.PIN_16, 9600,
    );
    let mut esc_0 = penguin_exp::dshot::PioDshot::new(
        &mut common, sm1,
        p.PIN_22, 52
    );
    for _i in 0..50 {
        esc_0.throttle(0).await;
        Timer::after_millis(50).await;
    }
    for _i in 0..10 {
        esc_0.direction(true).await;
        Timer::after_millis(50).await;
    }
    Timer::after_millis(400).await;

    info!("motor armed");

    let mut led = penguin_exp::blinker::Blinker::new(
        p.PIN_25,
        Duration::from_millis(100),
    );

    let mut adc = adc::Adc::new(p.ADC, Irqs, adc::Config::default());
    let mut potentiometer = penguin_exp::potentiometer::Potentiometer::new(p.PIN_29);
    let mut thermometer = penguin_exp::thermometer::Thermometer::new(p.ADC_TEMP_SENSOR);

    let mut ticker = Ticker::every(Duration::from_millis(200));
    const THROTTLE_MIN: u16 = 25;
    const THROTTLE_MAX: u16 = 40;
    let mut throttle = THROTTLE_MIN;
    let mut desc = false;
    while let () = ticker.next().await {
        if throttle <= THROTTLE_MIN {
            desc = false
        } else if throttle >= THROTTLE_MAX {
            desc = true;
        }
        throttle = if desc { throttle - 1 } else { throttle + 1 };
        esc_0.throttle(throttle).await;
        // led.blink().await;
        //
        // let vol = potentiometer.voltage(&mut adc).await.unwrap();
        // let temp = thermometer.temperature(&mut adc).await.unwrap();
        // let  _ = write!(frame, "vol: {}, temp: {} \r\n", vol, temp);
        // {
        //     use embedded_io_async::Write;
        //     uart_0.write(frame.as_bytes()).await.unwrap();
        // }
    }
}
