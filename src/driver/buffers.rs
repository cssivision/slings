use std::mem::ManuallyDrop;
use std::ops;

use bit_vec::BitVec;
use io_uring::opcode;

use crate::driver::Driver;

#[derive(Debug)]
pub struct Buffers {
    pub size: usize,
    pub num: usize,
    pub mem: *mut u8,
    bv: BitVec,
}

impl Buffers {
    pub fn new(num: usize, size: usize) -> Buffers {
        let total = num * size;
        let mut mem = ManuallyDrop::new(Vec::<u8>::with_capacity(total as usize));
        Buffers {
            mem: mem.as_mut_ptr(),
            num,
            size,
            bv: BitVec::from_elem(num, false),
        }
    }

    pub unsafe fn select(&mut self, bid: u16, driver: Driver) -> ProvidedBuf {
        let ptr = self.mem.add(self.size * bid as usize);
        let buf = ManuallyDrop::new(Vec::from_raw_parts(ptr, 0, self.size));
        if self.bv.get(bid as usize).unwrap_or_else(|| false) {
            panic!("buffer {} has selected", bid);
        }
        self.bv.set(bid as usize, true);
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

impl ProvidedBuf {
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.buf.set_len(new_len);
    }
}

impl Drop for ProvidedBuf {
    fn drop(&mut self) {
        if let Some(driver) = self.driver.take() {
            let driver = &mut *driver.inner.borrow_mut();
            let ring = &mut driver.ring;
            let buffers = &mut driver.buffers;
            let selected = buffers.bv.get(self.bid as usize).unwrap_or_else(|| false);
            if !selected {
                panic!("buffer {} not selected", self.bid);
            }
            let ptr = self.buf.as_mut_ptr();
            let entry = opcode::ProvideBuffers::new(ptr, buffers.size as _, 1, 0, self.bid)
                .build()
                .user_data(u64::MAX);

            if ring.submission().is_full() {
                ring.submit().expect("submit entry fail");
                ring.submission().sync();
            }
            unsafe {
                ring.submission().push(&entry).expect("push entry fail");
            }
            ring.submit().expect("submit entry fail");
            buffers.bv.set(self.bid as usize, false);
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
