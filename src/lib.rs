#![no_std]

use core::{fmt, future::Future, ptr::addr_of_mut};
use embedded_io::{Read, ReadReady, Write, WriteReady};
use nrf52833_hal::uarte::{Error, Instance, Uarte, UarteRx, UarteTx};

static mut TX_BUF: [u8; 1] = [0; 1];
static mut RX_BUF: [u8; 1] = [0; 1];

pub struct UartPort<T: Instance>(UarteTx<T>, UarteRx<T>);

impl<T: Instance> UartPort<T> {
    pub async fn read_async(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        for item in &mut *buffer {
            *item = ReadFuture { rx: &mut self.1 }.await?;
        }
        Ok(buffer.len())
    }

    pub fn read_until(
        &mut self,
        delimiter: u8,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let mut i = 0;
        for item in &mut *buffer {
            let mut buffer = [0];
            self.1.read(&mut buffer)?;
            *item = buffer[0];
            i += 1;
            if *item == delimiter {
                break;
            }
        }
        Ok(i)
    }

    pub async fn read_until_async(
        &mut self,
        delimiter: u8,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let mut i = 0;
        for item in &mut *buffer {
            *item = ReadFuture { rx: &mut self.1 }.await?;
            i += 1;
            if *item == delimiter {
                break;
            }
        }
        Ok(i)
    }
}

impl<T: Instance> TryFrom<Uarte<T>> for UartPort<T> {
    type Error = Error;

    fn try_from(value: Uarte<T>) -> Result<Self, Self::Error> {
        let (tx, rx) = value.split(unsafe { addr_of_mut!(TX_BUF).as_mut().unwrap() }, unsafe {
            addr_of_mut!(RX_BUF).as_mut().unwrap()
        })?;
        Ok(UartPort(tx, rx))
    }
}

impl<T: Instance> fmt::Write for UartPort<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)
    }
}

impl<T: Instance> embedded_io::ErrorType for UartPort<T> {
    type Error = Error;
}

impl<T: Instance> WriteReady for UartPort<T> {
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        self.0.write_ready()
    }
}

impl<T: Instance> Write for UartPort<T> {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, Self::Error> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush()
    }
}

impl<T: Instance> ReadReady for UartPort<T> {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        self.1.read_ready()
    }
}

impl<T: Instance> Read for UartPort<T> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.1.read(buffer)
    }
}

struct WriteFuture<'a, T: Instance> {
    tx: &'a mut UarteTx<T>,
    buffer: &'a [u8],
}

impl<T: Instance> Future for WriteFuture<'_, T> {
    type Output = Result<usize, Error>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context,
    ) -> core::task::Poll<Self::Output> {
        let this = self.get_mut();
        match this.tx.write_ready() {
            Ok(true) => match this.tx.write(this.buffer) {
                Ok(_) => core::task::Poll::Ready(Ok(this.buffer.len())),
                Err(e) => core::task::Poll::Ready(Err(e)),
            },
            Ok(false) => {
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            }
            Err(e) => core::task::Poll::Ready(Err(e)),
        }
    }
}

struct ReadFuture<'a, T: Instance> {
    rx: &'a mut UarteRx<T>,
}

impl<T: Instance> Future for ReadFuture<'_, T> {
    type Output = Result<u8, Error>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context,
    ) -> core::task::Poll<Self::Output> {
        let this = self.get_mut();
        match this.rx.read_ready() {
            Ok(true) => {
                let mut buffer = [0];
                match this.rx.read(&mut buffer) {
                    Ok(_) => core::task::Poll::Ready(Ok(buffer[0])),
                    Err(e) => core::task::Poll::Ready(Err(e)),
                }
            }
            Ok(false) => {
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            }
            Err(e) => core::task::Poll::Ready(Err(e)),
        }
    }
}
