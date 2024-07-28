#![no_std]
#![no_main]

use penguin_dshot::DshotTx;

use core::fmt::Write;
use core::sync::atomic::AtomicBool;
use heapless::String;

use embassy_executor::Spawner;
use embassy_rp::{adc, bind_interrupts, gpio};
use embassy_rp::{peripherals, pio};
use embassy_time::{Duration, Ticker, Timer};
use static_cell::StaticCell;

use defmt::{info, unwrap};
use embassy_rp::gpio::Pin;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

static PIO_0: StaticCell<peripherals::PIO0> = StaticCell::new();

static MOTOR_ENABLED: AtomicBool = AtomicBool::new(false);

#[embassy_executor::task]
async fn motor(mut esc_0: penguin_dshot::bidir::PioDshot<'static, peripherals::PIO0, 1>) {
    Timer::after_millis(800).await;
    esc_0.arm().await;

    let mut ticker = Ticker::every(Duration::from_millis(4 ));
    loop {
        let command = if MOTOR_ENABLED.load(core::sync::atomic::Ordering::Relaxed) {
            penguin_dshot::api::Command::Throttle(120)
        } else {
            penguin_dshot::api::Command::MotorStop
        };
        esc_0.send_command(command).await;
        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn button_task(pin: gpio::AnyPin) {
    let input = gpio::Input::new(pin, gpio::Pull::Up);
    let mut button = penguin_exp::button::Button::new(input, Duration::from_millis(40));
    loop {
        let level = button.debounce().await;
        if level != gpio::Level::High {
            continue;
        }
        info!("button");
        let enabled = MOTOR_ENABLED.load(core::sync::atomic::Ordering::Relaxed);
        MOTOR_ENABLED.store(!enabled, core::sync::atomic::Ordering::Relaxed);
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
        mut common,
        sm0,
        sm1,
        ..
    } = pio::Pio::new(pio_0, Irqs);
    let mut uart_0 = penguin_exp::uart::PioUartTx::new(&mut common, sm0, p.PIN_16, 9600);
    let esc_0 = penguin_dshot::bidir::PioDshot::new(&mut common, sm1, p.PIN_22);
    unwrap!(spawner.spawn(motor(esc_0)));
    let pin_btn = p.PIN_1.degrade();
    unwrap!(spawner.spawn(button_task(pin_btn)));

    let mut led = penguin_exp::blinker::Blinker::new(p.PIN_25, Duration::from_millis(100));

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
        let _ = write!(frame, "vol: {}, temp: {} \r\n", vol, temp);
        {
            use embedded_io_async::Write;
            uart_0.write(frame.as_bytes()).await.unwrap();
        }
    }
}
