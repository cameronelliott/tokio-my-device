use std::io::{Error as IoError, Result};
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::future::poll_fn;
use futures_core::{ready, Future};
use timerfd::{SetTimeFlags, TimerFd as InnerTimerFd, TimerState};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, Interest, ReadBuf};

pub use timerfd::ClockId;
use tokio::select;
use tokio::time::Interval;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let mut a = X::new(ClockId::Monotonic).unwrap();
    a.set_state(
        TimerState::Oneshot(Duration::from_secs(3)),
        SetTimeFlags::Default,
    );

    let mut b = X::new(ClockId::Monotonic).unwrap();
    b.set_state(
        TimerState::Oneshot(Duration::from_secs(5)),
        SetTimeFlags::Default,
    );

    println!("before sel");

    let c = tokio::time::sleep(Duration::from_secs(1));

    let start = Instant::now();
    _ = tokio::join!(
        async {
            a.await.unwrap();
            println!("a done")
        },
        async {
            b.await.unwrap();
            println!("b done")
        },
        async {
            c.await;
            println!("c done")
        },
    );
    let duration = start.elapsed();

    println!("all futures done {:?}", duration);

    let mut d = X::new(ClockId::Monotonic).unwrap();
    d.set_state(
        TimerState::Periodic {
            current: Duration::from_secs(2),
            interval: Duration::from_secs(2),
        },
        SetTimeFlags::Default,
    );

    d.tick().await;
    println!("tick");
    d.tick().await;
    println!("tick");
    d.tick().await;
    println!("tick");
    d.tick().await;
    println!("tick");

    // tokio::select! {
    //     val = a => {
    //         println!("a completed first with {:?}", val);
    //     }
    //     val = b => {
    //         println!("b completed first with {:?}", val);
    //     }
    //     val = c => {
    //         println!("c completed first with {:?}", val);
    //     }
    // }

    // x.await?;

    println!("done waiting");
    Ok(())
}

pub struct X(AsyncFd<InnerTimerFd>);

impl X {
    pub fn new(clock: ClockId) -> std::io::Result<Self> {
        let fd = InnerTimerFd::new_custom(clock, true, true)?;
        let inner = AsyncFd::with_interest(fd, Interest::READABLE)?;
        Ok(X(inner))
    }

    fn set_state(&mut self, state: TimerState, flags: SetTimeFlags) {
        (self.0).get_mut().set_state(state, flags);
    }

    pub async fn tick(&mut self) {
       self.await;
    }
}



fn read_u64(fd: RawFd) -> Result<u64> {
    let mut buf = [0u8; 8];
    let rv = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, 8) };
    match rv {
        len if len >= 0 => Ok(u64::from_ne_bytes(buf)),
        _err => Err(IoError::last_os_error()),
    }
}

impl AsyncRead for X {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let inner = self.as_mut();
        let fd = inner.0.as_raw_fd();

        loop {
            let mut guard = ready!(inner.0.poll_read_ready(cx))?;

            match guard.try_io(|_| read_u64(fd)) {
                Ok(res) => {
                    let num = res?;
                    buf.put_slice(&num.to_ne_bytes());
                    break;
                }
                Err(_) => continue,
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl Future for X {
    type Output = std::io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // if !self.initialized {
        //     let now = Instant::now();
        //     let duration = if self.deadline > now {
        //         self.deadline - now
        //     } else {
        //         return Poll::Ready(Ok(()));
        //     };
        //     self.timerfd
        //         .set_state(TimerState::Oneshot(duration), SetTimeFlags::Default);
        //     self.initialized = true;
        // }
        let mut buf = [0u8; 8];
        let mut buf = ReadBuf::new(&mut buf);
        ready!(Pin::new(&mut self.as_mut()).poll_read(cx, &mut buf)?);
        Poll::Ready(Ok(()))
    }
}
