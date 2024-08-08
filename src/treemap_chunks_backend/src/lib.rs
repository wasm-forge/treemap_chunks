use std::{cell::RefCell, ops::Range};

use ic_cdk::api::stable::WASM_PAGE_SIZE_IN_BYTES;
use ic_stable_structures::{memory_manager::{MemoryId, MemoryManager}, storable::Bound, DefaultMemoryImpl, StableBTreeMap};

use ic_stable_structures::memory_manager::VirtualMemory;

 
const CHUNK_SIZE: usize = 4096;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
}


type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Clone)]
struct MyChunk(Vec<u8>);

impl ic_stable_structures::Storable for MyChunk {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Self(<Vec<u8>>::from_bytes(bytes))
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 101_000_000,
        is_fixed_size: false
    };
}


#[derive(Clone)]
struct MyChunk4k(Vec<u8>);

impl ic_stable_structures::Storable for MyChunk4k {

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        self.0.to_bytes()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Self(<Vec<u8>>::from_bytes(bytes))
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: CHUNK_SIZE as u32,
        is_fixed_size: false
    };
}


thread_local! {
    static BUFFER: RefCell<Option<Vec<u8>>> = RefCell::new(None);


    static MAP: RefCell<StableBTreeMap<u64, MyChunk, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(110))))
    );

    static MAP4K: RefCell<StableBTreeMap<(u64, u64), MyChunk4k, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(111))))
    );

    static CUSTOM: RefCell<Memory> = RefCell::new(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(116))));

}


#[ic_cdk::update]
pub fn append_buffer(text: String, times: usize) -> usize {

    let res = BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        let total_length = text.len() * times;

        if buf.is_none() {
            *buf = Some(Vec::with_capacity(total_length));
        }

        let buf = buf.as_mut().unwrap();

        for _ in 0..times {
            buf.extend_from_slice(&text.as_ref());
        }

        buf.len()
    });

    res
}


#[ic_cdk::update]
pub fn store_buffer(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|buf| {

        let buf = buf.borrow_mut();

        let buf = buf.as_ref();
        
        let buf = buf.unwrap();

        let len = (*buf).len();

        MAP.with(|mp| {

            let mut mp = mp.borrow_mut();

            mp.insert(key, MyChunk((*buf).clone()));
        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}


#[ic_cdk::update]
pub fn store_memory(offset: u64) -> (u64, usize) {
    use ic_stable_structures::Memory;

    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|buf| {

        let buf = buf.borrow_mut();

        let buf = buf.as_ref();
        
        let buf = buf.unwrap();

        let len = (*buf).len();

        CUSTOM.with(|mp| {

            let mp = mp.borrow_mut();

            let max_address = offset + len as u64;

            let pages_required = (max_address + WASM_PAGE_SIZE_IN_BYTES - 1) / WASM_PAGE_SIZE_IN_BYTES;

            let cur_pages = mp.size();
        
            if cur_pages < pages_required {
                mp.grow(pages_required - cur_pages);
            }

            mp.write(offset, buf);

        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}


#[ic_cdk::update]
pub fn store_buffer_4k(key: u64) -> (u64, usize, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let (res, idx) = BUFFER.with(|buf| {

        let buf = buf.borrow_mut();

        let buf = buf.as_ref();
        
        let buf = buf.unwrap();

        let mut len = 0;

        let mut idx = 0;

        MAP4K.with(|mp| {
            let mut mp = mp.borrow_mut();

            loop {
                let upper = std::cmp::min((&buf).len(), ((idx+1)*CHUNK_SIZE) as usize);
                let lower = std::cmp::min((&buf).len(), (idx*CHUNK_SIZE) as usize);

                if lower==upper {
                    break;
                }

                let slice = &buf[lower..upper];

                let mut vec: Vec<u8> = Vec::with_capacity(CHUNK_SIZE);
                vec.extend_from_slice(slice);

                len += vec.len();

                if vec.len() > 0 {
                    mp.insert((key, idx as u64), MyChunk4k(vec));
                } else {
                    break;
                }
    
                idx += 1;
            };
        });

        (len, idx)
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res, idx)
}

////////////////////////////////////////////////////////////////////////////////////////////////////////
/// 

#[ic_cdk::update]
pub fn load_buffer(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|buf| {

        let mut buf = buf.borrow_mut();
        
        MAP.with(|mp| {

            let mp = mp.borrow_mut();

            let read = mp.get(&key).unwrap();

            *buf = Some(read.0);
        });

        (*buf).as_ref().unwrap().len()
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}


#[ic_cdk::update]
pub fn load_buffer_4k(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|buf| {

        let mut buf = buf.borrow_mut();

        if buf.is_none() {
            *buf = Some(Vec::new());
        }

        let buf = buf.as_mut().unwrap();
        
        let mut len = 0;

        let mut idx = 0;

        MAP4K.with(|mp| {
    
            let mp = mp.borrow_mut();

            loop {
                let read = mp.get(&(key, idx));

                if let Some(chunk) = read {
                    len += chunk.0.len();
                    buf.extend_from_slice(&chunk.0[..]);
                    idx += 1;
                } else {
                    break;
                }
            }

        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}


#[ic_cdk::update]
pub fn load_buffer_4k_ranged(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();

    let res = BUFFER.with(|buf| {

        let mut buf = buf.borrow_mut();

        if buf.is_none() {
            *buf = Some(Vec::new());
        }

        let buf = buf.as_mut().unwrap();
        
        let mut len = 0;

        MAP4K.with(|mp| {
    
            let mp = mp.borrow_mut();

            let range = Range { 
                start: (key, 0), 
                end: (key + 1, 0)
            };

            let iter = mp.range(range);

            for ((_, _idx), chunk) in iter {
                len += chunk.0.len();
                buf.extend_from_slice(&chunk.0[..]);
            }

        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}

////////////////////////////////////////////////////////////

#[ic_cdk::init]
fn init() {
    //profiling_init();
}


#[ic_cdk::update]
pub fn clear_buffer() {

    BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        if buf.is_none() {
            return;
        }

        let buf = buf.as_mut().unwrap();

        buf.clear()
    })
}


#[ic_cdk::update]
pub fn zero_buffer() {

    BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        if buf.is_none() {
            return;
        }

        let buf = buf.as_mut().unwrap();

        // explicitly destroy contents
        for i in 0..buf.len() {
            buf[i] = 0;
        }

    })
}


#[ic_cdk::update]
pub fn read_buffer(offset: usize, size: usize) -> String {

    BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        let buf = buf.as_mut().unwrap();

        std::str::from_utf8(&buf[offset..offset+size]).unwrap().to_string()
    })
}


#[ic_cdk::update]
pub fn chunk_size() -> usize {

    BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        let buf = buf.as_mut().unwrap();

        buf.len()
    })
}
