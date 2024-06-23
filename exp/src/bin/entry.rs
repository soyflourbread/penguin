#![no_std]
#![no_main]

use core::fmt::Write;
use heapless::String;

use embassy_executor::Spawner;
use embassy_rp::{adc, bind_interrupts, gpio, pwm};
use embassy_rp::{pio, i2c, peripherals, uart};
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;

use defmt::{info, unwrap};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

static PIO_0: StaticCell<peripherals::PIO0> = StaticCell::new();

#[embassy_executor::task]
async fn motor(
    mut esc_0: penguin_exp::dshot::PioDshot<'static, peripherals::PIO0, 1>
) {
    esc_0.arm().await;
    info!("motor armed");

    const THROTTLE_MIN: u16 = 100;
    const THROTTLE_MAX: u16 = 200;
    let mut throttle = THROTTLE_MIN;
    let mut desc = false;

    let mut ticker = Ticker::every(Duration::from_millis(10));
    loop {
        esc_0.beep().await;
        // ticker.next().await;
        // if throttle <= THROTTLE_MIN {
        //     desc = false
        // } else if throttle >= THROTTLE_MAX {
        //     desc = true;
        // }
        // throttle = if desc { throttle - 1 } else { throttle + 1 };
        // esc_0.throttle(throttle).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // let servo_0 = penguin_exp::servo::ServoAB::new(
    //     p.PWM_SLICE1, p.PIN_18, p.PIN_19,
    //     20.0, 1.2, 1.8, 0.5
    // ); // datasheet: 0.5ms-2.5ms

    let pio_0: &'static mut _ = PIO_0.init(p.PIO0);
    let pio::Pio {
        mut common, sm0, sm1, ..
    } = pio::Pio::new(pio_0, Irqs);
    // let mut uart_0 = penguin_exp::uart::PioUartTx::new(
    //     &mut common, sm0,
    //     p.PIN_16, 9600,
    // );
    let esc_0 = penguin_exp::dshot::PioDshot::new(
        &mut common, sm1,
        p.PIN_22,
    );
    unwrap!(spawner.spawn(motor(esc_0)));

    let mut led = penguin_exp::blinker::Blinker::new(
        p.PIN_25,
        Duration::from_millis(100),
    );

    let mut adc = adc::Adc::new(p.ADC, Irqs, adc::Config::default());
    let mut potentiometer = penguin_exp::potentiometer::Potentiometer::new(p.PIN_29);
    let mut thermometer = penguin_exp::thermometer::Thermometer::new(p.ADC_TEMP_SENSOR);

    let mut ticker = Ticker::every(Duration::from_millis(800));
    let mut frame: String<128> = String::new();
    loop {
        ticker.next().await;
        led.blink().await;

        let vol = potentiometer.voltage(&mut adc).await.unwrap();
        let temp = thermometer.temperature(&mut adc).await.unwrap();
        frame.clear();
        // let  _ = write!(frame, "vol: {}, temp: {} \r\n", vol, temp);
        // {
        //     use embedded_io_async::Write;
        //     uart_0.write(frame.as_bytes()).await.unwrap();
        // }
    }
}
