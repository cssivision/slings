use std::mem::ManuallyDrop;
use std::ops;

use io_uring::opcode;

use crate::driver::Driver;

pub const GROUP_ID: u16 = 1337;

#[derive(Debug)]
pub struct Buffers {
    pub size: usize,
    pub num: usize,
    pub mem: *mut u8,
}

impl Buffers {
    pub fn new(num: usize, size: usize) -> Buffers {
        let total = num * size;
        let mut mem = ManuallyDrop::new(Vec::<u8>::with_capacity(total as usize));
        Buffers {
            mem: mem.as_mut_ptr(),
            num,
            size,
        }
    }

    pub unsafe fn select(&mut self, bid: u16, driver: Driver) -> ProvidedBuf {
        let ptr = self.mem.add(self.size * bid as usize);
        let buf = ManuallyDrop::new(Vec::from_raw_parts(ptr, 0, self.size));
        ProvidedBuf {
            buf,
            driver: Some(driver),
            bid,
        }
    }
}

pub struct ProvidedBuf {
    buf: ManuallyDrop<Vec<u8>>,
    driver: Option<Driver>,
    bid: u16,
}

impl Drop for ProvidedBuf {
    fn drop(&mut self) {
        if let Some(driver) = self.driver.take() {
            let driver = &mut *driver.inner.borrow_mut();
            let buffers = &mut driver.buffers;
            let entry = opcode::ProvideBuffers::new(
                self.buf.as_mut_ptr(),
                buffers.size as _,
                1,
                GROUP_ID,
                self.bid,
            )
            .build()
            .user_data(u64::MAX);

            let ring = &mut driver.ring;
            if ring.submission().is_full() {
                ring.submit().expect("submit fail");
                ring.submission().sync();
            }
            unsafe {
                ring.submission().push(&entry).expect("push entry fail");
            }
            ring.submit().expect("submit fail");
        }
    }
}

impl Default for ProvidedBuf {
    fn default() -> Self {
        ProvidedBuf {
            buf: ManuallyDrop::new(Vec::new()),
            driver: None,
            bid: 0,
        }
    }
}

impl ops::Deref for ProvidedBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf[..]
    }
}

impl ops::DerefMut for ProvidedBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buf[..]
    }
}
