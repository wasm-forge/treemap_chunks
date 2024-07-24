use std::cell::RefCell;

use ic_stable_structures::{memory_manager::{MemoryId, MemoryManager}, storable::Bound, DefaultMemoryImpl, StableBTreeMap};

use ic_stable_structures::memory_manager::VirtualMemory;


#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

const PROFILING: MemoryId = MemoryId::new(100);

pub fn profiling_init() {
    let memory = MEMORY_MANAGER.with(|m| m.borrow().get(PROFILING));
    ic_stable_structures::Memory::grow(&memory, 4096);
}

////////////////////////////////////////
 
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
        max_size: 150_000_000,
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
    static BUFFER: RefCell<Option<MyChunk>> = RefCell::new(None);
    
    static MAP: RefCell<StableBTreeMap<u64, MyChunk, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(110))))
    );

    static MAP4K: RefCell<StableBTreeMap<(u64, u64), MyChunk4k, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(111))))
    );
}


#[ic_cdk::update]
pub fn append_chunk(text: String, times: usize) -> usize {

    let res = BUFFER.with(|chunk| {
        let mut chunk = chunk.borrow_mut();

        let total_length = text.len() * times;

        if chunk.is_none() {
            *chunk = Some(MyChunk(Vec::with_capacity(total_length)));
        }

        let chunk = chunk.as_mut().unwrap();

        for _ in 0..times {
            chunk.0.extend_from_slice(&text.as_ref());
        }

        chunk.0.len()
    });

    res
}

#[ic_cdk::update]
pub fn store_chunk(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|chunk| {

        let chunk = chunk.borrow_mut();

        let chunk = chunk.as_ref();
        
        let chunk = chunk.unwrap();

        let len = (*chunk).0.len();

        MAP.with(|mp| {

            let mut mp = mp.borrow_mut();

            mp.insert(key, (*chunk).clone());
        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}

#[ic_cdk::update]
pub fn store_chunk_4k(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|chunk| {

        let mut chunk = chunk.borrow_mut();

        let chunk = chunk.take();
        
        let chunk = chunk.unwrap();

        let mut len = 0;

        let mut idx = 0;

        MAP4K.with(|mp| {
            let mut mp = mp.borrow_mut();

            loop {

                let upper = std::cmp::min((&chunk.0).len(), ((idx+1)*CHUNK_SIZE) as usize);
                let lower = std::cmp::min((&chunk.0).len(), (idx*CHUNK_SIZE) as usize);

                if lower==upper {
                    break;
                }

                let slice = &chunk.0[lower..upper];

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

        len
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}

#[ic_cdk::update]
pub fn load_chunk_map(key: u64) -> (u64, usize) {
    let stime = ic_cdk::api::instruction_counter();    

    let res = BUFFER.with(|chunk| {

        let mut chunk = chunk.borrow_mut();
        
        MAP.with(|mp| {

            let mp = mp.borrow_mut();

            let read = mp.get(&key).unwrap();

            *chunk = Some(read);
        });

        (*chunk).as_ref().unwrap().0.len()
    });

    let etime = ic_cdk::api::instruction_counter();    

    (etime - stime, res)
}

////////////////////////////////////////////////////////////

#[ic_cdk::init]
fn init() {
    profiling_init();
}


#[ic_cdk::update]
pub fn clear_chunk() {

    BUFFER.with(|chunk| {
        let mut chunk = chunk.borrow_mut();

        if chunk.is_none() {
            return;
        }

        let chunk = chunk.as_mut().unwrap();

        chunk.0.clear()
    })
}

#[ic_cdk::update]
pub fn zero_chunk() {

    BUFFER.with(|chunk| {
        let mut chunk = chunk.borrow_mut();

        if chunk.is_none() {
            return;
        }

        let chunk = chunk.as_mut().unwrap();

        // explicitly destroy contents
        for i in 0..chunk.0.len() {
            chunk.0[i] = 0;
        }

    })
}

#[ic_cdk::update]
pub fn read_chunk(offset: usize, size: usize) -> String {

    BUFFER.with(|chunk| {
        let mut chunk = chunk.borrow_mut();

        let chunk = chunk.as_mut().unwrap();

        std::str::from_utf8(&chunk.0[offset..offset+size]).unwrap().to_string()
    })
}

#[ic_cdk::update]
pub fn chunk_size() -> usize {

    BUFFER.with(|chunk| {
        let mut chunk = chunk.borrow_mut();

        let chunk = chunk.as_mut().unwrap();

        chunk.0.len()
    })
}
