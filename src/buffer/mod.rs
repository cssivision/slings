use std::cell::Cell;
use std::fmt;
use std::io;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::sync::atomic;
use std::sync::atomic::AtomicU16;

use io_uring::types::BufRingEntry;

type Bgid = u16; // Buffer group id
type Bid = u16; // Buffer id

// The Builder API for a BufRing.
#[derive(Copy, Clone)]
pub(crate) struct Builder {
    bgid: Bgid,
    ring_entries: u16,
    buf_cnt: u16,
    buf_len: usize,
}

impl Builder {
    // Create a new Builder with the given buffer group ID and defaults.
    //
    // The buffer group ID, `bgid`, is the id the kernel uses to identify the buffer group to use
    // for a given read operation that has been placed into an sqe.
    //
    // The caller is responsible for picking a bgid that does not conflict with other buffer
    // groups that have been registered with the same uring interface.
    pub fn new(bgid: Bgid) -> Builder {
        Builder {
            bgid,
            ring_entries: 128,
            buf_cnt: 0, // 0 indicates buf_cnt is taken from ring_entries
            buf_len: 4096,
        }
    }

    // The number of ring entries to create for the buffer ring.
    //
    // The number will be made a power of 2, and will be the maximum of the ring_entries setting
    // and the buf_cnt setting. The interface will enforce a maximum of 2^15 (32768).
    pub fn ring_entries(mut self, ring_entries: u16) -> Builder {
        self.ring_entries = ring_entries;
        self
    }

    // The number of buffers to allocate. If left zero, the ring_entries value will be used.
    pub fn buf_cnt(mut self, buf_cnt: u16) -> Builder {
        self.buf_cnt = buf_cnt;
        self
    }

    // The length to be preallocated for each buffer.
    pub fn buf_len(mut self, buf_len: usize) -> Builder {
        self.buf_len = buf_len;
        self
    }

    // Return a BufRing.
    pub fn build(&self) -> io::Result<BufRing> {
        let mut b: Builder = *self;

        // Two cases where both buf_cnt and ring_entries are set to the max of the two.
        if b.buf_cnt == 0 || b.ring_entries < b.buf_cnt {
            let max = std::cmp::max(b.ring_entries, b.buf_cnt);
            b.buf_cnt = max;
            b.ring_entries = max;
        }

        // Don't allow the next_power_of_two calculation to be done if already larger than 2^15
        // because 2^16 reads back as 0 in a u16. The interface doesn't allow for ring_entries
        // larger than 2^15 anyway, so this is a good place to catch it. Here we return a unique
        // error that is more descriptive than the InvalidArg that would come from the interface.
        if b.ring_entries > (1 << 15) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "ring_entries exceeded 32768",
            ));
        }

        // Requirement of the interface is the ring entries is a power of two, making its and our
        // wrap calculation trivial.
        b.ring_entries = b.ring_entries.next_power_of_two();

        let inner = InnerBufRing::new(b.bgid, b.ring_entries, b.buf_cnt, b.buf_len)?;
        Ok(BufRing {
            inner: Rc::new(inner),
        })
    }
}

// The BufRing is reference counted because each buffer handed
// out has a reference back to its buffer ring.
#[derive(Clone)]
pub(crate) struct BufRing {
    inner: Rc<InnerBufRing>,
}

impl BufRing {
    // Returns the buffer the uring interface picked from the buf_ring for the completion result
    // represented by the res and flags.
    pub fn get_buf(&self, len: usize, bid: u16) -> io::Result<GBuf> {
        self.inner.get_buf(self.clone(), len, bid)
    }

    // Returns the buffer group id.
    pub fn bgid(&self) -> Bgid {
        self.inner.bgid()
    }

    pub fn ring_entries(&self) -> u16 {
        self.inner.ring_entries()
    }

    /// Get a pointer to the memory.
    pub fn as_ptr(&self) -> *const libc::c_void {
        self.inner.ring_start.as_ptr()
    }
}

// This tracks a buffer that has been filled in by the kernel, having gotten the memory
// from a buffer ring, and returned to userland via a cqe entry.
pub(crate) struct GBuf {
    bufgroup: BufRing,
    len: usize,
    bid: Bid,
}

impl GBuf {
    fn new(bufgroup: BufRing, bid: Bid, len: usize) -> Self {
        assert!(len <= bufgroup.inner.buf_capacity());
        Self { bufgroup, len, bid }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    // Return a byte slice reference.
    pub(crate) fn as_slice(&self) -> &[u8] {
        let p = self.bufgroup.inner.stable_ptr(self.bid);
        unsafe { std::slice::from_raw_parts(p, self.len) }
    }
}

impl fmt::Debug for GBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GBuf")
            .field("bgid", &self.bufgroup.inner.bgid())
            .field("bid", &self.bid)
            .field("len", &self.len)
            .field("cap", &self.bufgroup.inner.buf_capacity())
            .finish()
    }
}

impl Drop for GBuf {
    fn drop(&mut self) {
        // Add the buffer back to the bufgroup, for the kernel to reuse.
        unsafe { self.bufgroup.inner.dropping_bid(self.bid) };
    }
}

// All these fields are constant once the struct is instantiated except the one of type Cell<u16>.
struct InnerBufRing {
    bgid: Bgid,

    ring_entries_mask: u16, // Invariant one less than ring_entries which is > 0, power of 2, max 2^15 (32768).

    buf_cnt: u16,   // Invariants: > 0, <= ring_entries.
    buf_len: usize, // Invariant: > 0.

    // `ring_start` holds the memory allocated for the buf_ring, the ring of entries describing
    // the buffers being made available to the uring interface for this buf group id.
    ring_start: Mmap,

    buf_list: Vec<Vec<u8>>,

    // `local_tail` is the copy of the tail index that we update when a buffer is dropped and
    // therefore its buffer id is released and added back to the ring. It also serves for adding
    // buffers to the ring during init but that's not as interesting.
    local_tail: Cell<u16>,

    // `shared_tail` points to the u16 memory inside the rings that the uring interface uses as the
    // tail field. It is where the application writes new tail values and the kernel reads the tail
    // value from time to time. The address could be computed from ring_start when needed. This
    // might be here for no good reason any more.
    shared_tail: *const AtomicU16,
}

impl InnerBufRing {
    fn new(
        bgid: Bgid,
        ring_entries: u16,
        buf_cnt: u16,
        buf_len: usize,
    ) -> io::Result<InnerBufRing> {
        // Check that none of the important args are zero and the ring_entries is at least large
        // enough to hold all the buffers and that ring_entries is a power of 2.
        if (buf_cnt == 0)
            || (buf_cnt > ring_entries)
            || (buf_len == 0)
            || ((ring_entries & (ring_entries - 1)) != 0)
        {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        // entry_size is 16 bytes.
        let entry_size = mem::size_of::<BufRingEntry>();
        assert_eq!(entry_size, 16);
        let ring_size = entry_size * (ring_entries as usize);
        let ring_start = Mmap::new(ring_size)?;
        ring_start.dontfork()?;

        // Probably some functional way to do this.
        let buf_list: Vec<Vec<u8>> = {
            let mut bp = Vec::with_capacity(buf_cnt as _);
            for _ in 0..buf_cnt {
                bp.push(vec![0; buf_len]);
            }
            bp
        };

        let shared_tail = unsafe { BufRingEntry::tail(ring_start.as_ptr() as *const BufRingEntry) }
            as *const AtomicU16;

        let buf_ring = InnerBufRing {
            bgid,
            ring_entries_mask: ring_entries - 1,
            buf_cnt,
            buf_len,
            ring_start,
            buf_list,
            local_tail: Cell::new(0),
            shared_tail,
        };

        for bid in 0..buf_cnt {
            buf_ring.push(bid);
        }
        buf_ring.sync();
        Ok(buf_ring)
    }

    // Push the `bid` buffer to the buf_ring tail.
    // This test version does not safeguard against a duplicate
    // `bid` being pushed.
    fn push(&self, bid: Bid) {
        assert!(bid < self.buf_cnt);

        // N.B. The uring buf_ring indexing mechanism calls for the tail values to exceed the
        // actual number of ring entries. This allows the uring interface to distinguish between
        // empty and full buf_rings. As a result, the ring mask is only applied to the index used
        // for computing the ring entry, not to the tail value itself.

        let old_tail = self.local_tail.get();
        self.local_tail.set(old_tail + 1);
        let ring_idx = old_tail & self.mask();

        let entries = self.ring_start.as_mut_ptr() as *mut BufRingEntry;
        let re = unsafe { &mut *entries.add(ring_idx as usize) };

        re.set_addr(self.stable_ptr(bid) as _);
        re.set_len(self.buf_len as _);
        re.set_bid(bid);

        // Also note, we have not updated the tail as far as the kernel is concerned.
        // That is done with buf_ring_sync.
    }

    // Make 'local_tail' visible to the kernel. Called after buf_ring_push() has been
    // called to fill in new buffers.
    fn sync(&self) {
        unsafe {
            (*self.shared_tail).store(self.local_tail.get(), atomic::Ordering::Release);
        }
    }

    // Safety: dropping a duplicate bid is likely to cause undefined behavior
    // as the kernel could use the same buffer for different data concurrently.
    unsafe fn dropping_bid(&self, bid: Bid) {
        self.push(bid);
        self.sync();
    }

    fn stable_ptr(&self, bid: Bid) -> *const u8 {
        self.buf_list[bid as usize].as_ptr()
    }

    fn ring_entries(&self) -> u16 {
        self.ring_entries_mask + 1
    }

    fn mask(&self) -> u16 {
        self.ring_entries_mask
    }

    fn buf_capacity(&self) -> usize {
        self.buf_len as _
    }

    // Returns the buffer group id.
    fn bgid(&self) -> Bgid {
        self.bgid
    }

    // Returns the buffer the uring interface picked from the buf_ring for the completion result
    // represented by the res and flags.
    fn get_buf(&self, buf_ring: BufRing, len: usize, bid: u16) -> io::Result<GBuf> {
        // This fn does the odd thing of having self as the BufRing and taking an argument that is
        // the same BufRing but wrapped in Rc<_> so the wrapped buf_ring can be passed to the
        // outgoing GBuf.
        assert!(len <= self.buf_len);
        Ok(GBuf::new(buf_ring, bid, len))
    }
}

/// A region of memory mapped using `mmap(2)`.
struct Mmap {
    addr: ptr::NonNull<libc::c_void>,
    len: usize,
}

impl Mmap {
    /// Map `len` bytes into memory.
    fn new(len: usize) -> io::Result<Mmap> {
        unsafe {
            match libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_POPULATE,
                -1,
                0,
            ) {
                libc::MAP_FAILED => Err(io::Error::last_os_error()),
                addr => {
                    // here, `mmap` will never return null
                    let addr = ptr::NonNull::new_unchecked(addr);
                    Ok(Mmap { addr, len })
                }
            }
        }
    }

    /// Do not make the stored memory accessible by child processes after a `fork`.
    fn dontfork(&self) -> io::Result<()> {
        match unsafe { libc::madvise(self.addr.as_ptr(), self.len, libc::MADV_DONTFORK) } {
            0 => Ok(()),
            _ => Err(io::Error::last_os_error()),
        }
    }

    /// Get a pointer to the memory.
    #[inline]
    pub fn as_ptr(&self) -> *const libc::c_void {
        self.addr.as_ptr()
    }

    /// Get a pointer to the memory.
    #[inline]
    fn as_mut_ptr(&self) -> *mut libc::c_void {
        self.addr.as_ptr()
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.addr.as_ptr(), self.len);
        }
    }
}
