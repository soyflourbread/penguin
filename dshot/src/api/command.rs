#[derive(Debug)]
pub enum CommandError {
    Invalid,
}

pub enum Command {
    MotorStop,
    ExtendedTelemetry { enabled: bool },
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
            Command::ExtendedTelemetry { enabled } => { if enabled { 13 } else { 14 } },
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
