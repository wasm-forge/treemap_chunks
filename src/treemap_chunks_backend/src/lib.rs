use std::{cell::RefCell, ops::Range};

use ic_cdk::export_candid;
use ic_cdk::stable::WASM_PAGE_SIZE_IN_BYTES;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap,
};

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

    fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Self(<Vec<u8>>::from_bytes(bytes))
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 101_000_000,
        is_fixed_size: false,
    };
}

#[derive(Clone)]
struct MyChunk4k(Vec<u8>);

impl ic_stable_structures::Storable for MyChunk4k {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        self.0.to_bytes()
    }

    fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Self(<Vec<u8>>::from_bytes(bytes))
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: CHUNK_SIZE as u32,
        is_fixed_size: false,
    };
}

thread_local! {
    static BUFFER: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };


    static MAP: RefCell<StableBTreeMap<u64, MyChunk, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(110))))
    );

    static MAP4K: RefCell<StableBTreeMap<(u64, u64), MyChunk4k, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(111))))
    );

    static TEST_MAP: RefCell<StableBTreeMap<u64, String, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(112))))
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
            buf.extend_from_slice(text.as_ref());
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

        let len: usize = (*buf).len();

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
pub fn store_into_memory(offset: u64) -> (u64, u64) {
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

            let pages_required = max_address.div_ceil(WASM_PAGE_SIZE_IN_BYTES);

            let cur_pages = mp.size();

            if cur_pages < pages_required {
                mp.grow(pages_required - cur_pages);
            }

            mp.write(offset, buf);
        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();

    (etime - stime, res as u64)
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
                let upper = std::cmp::min(buf.len(), (idx + 1) * CHUNK_SIZE);
                let lower = std::cmp::min(buf.len(), idx * CHUNK_SIZE);

                if lower == upper {
                    break;
                }

                let slice = &buf[lower..upper];

                let mut vec: Vec<u8> = Vec::with_capacity(CHUNK_SIZE);
                vec.extend_from_slice(slice);

                len += vec.len();

                if !vec.is_empty() {
                    mp.insert((key, idx as u64), MyChunk4k(vec));
                } else {
                    break;
                }

                idx += 1;
            }
        });

        (len, idx)
    });

    let etime = ic_cdk::api::instruction_counter();

    (etime - stime, res, idx)
}

////////////////////////////////////////////////////////////////////////////////////////////////////////

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
                end: (key + 1, 0),
            };

            let iter = mp.range(range);

            for en in iter {
                let chunk = en.value();

                len += chunk.0.len();
                buf.extend_from_slice(&chunk.0[..]);
            }
        });

        len
    });

    let etime = ic_cdk::api::instruction_counter();

    (etime - stime, res)
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

        let buf: &mut Vec<u8> = buf.as_mut().unwrap();

        buf.fill(0);
    })
}

#[ic_cdk::update]
pub fn read_buffer(offset: usize, size: usize) -> String {
    BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        let buf = buf.as_mut().unwrap();

        std::str::from_utf8(&buf[offset..offset + size])
            .unwrap()
            .to_string()
    })
}

#[ic_cdk::update]
pub fn buffer_size() -> usize {
    BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();

        let buf = buf.as_mut().unwrap();

        buf.len()
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs, sync::Once};

    use crate::TEST_MAP;
    use pocket_ic::PocketIc;
    const WASM_PATH: &str =
        "../../target/wasm32-unknown-unknown/release/treemap_chunks_backend.wasm";
    use candid::Principal;

    thread_local!(
        static ACTIVE_CANISTER: RefCell<Option<Principal>> = const { RefCell::new(None) };
    );

    fn set_active_canister(new_canister: Principal) {
        ACTIVE_CANISTER.with(|canister_cell| {
            let mut canister = canister_cell.borrow_mut();
            *canister = Some(new_canister);
        })
    }

    fn active_canister() -> Principal {
        ACTIVE_CANISTER.with(|canister_cell| {
            let canister = *canister_cell.borrow();
            canister.unwrap()
        })
    }

    static INIT: Once = Once::new();

    fn build_test_project() {
        INIT.call_once(|| {
            use std::process::Command;

            let _ = Command::new("bash")
                .arg("../../build.sh")
                .output()
                .expect("Failed to execute command");
        });
    }

    fn setup_initial_canister() -> PocketIc {
        build_test_project();

        let pic = PocketIc::new();
        //        let pic = PocketIcBuilder::new().with_benchmarking_system_subnet().build();

        let wasm = fs::read(WASM_PATH).expect("Wasm file not found, run 'dfx build'.");

        let backend_canister = pic.create_canister();

        pic.add_cycles(backend_canister, 1_000_000_000_000_000);

        set_active_canister(backend_canister);

        pic.install_canister(backend_canister, wasm, vec![], None);

        pic.tick();

        pic
    }

    mod fns {

        use candid::Principal;
        use pocket_ic::PocketIc;

        use super::active_canister;

        pub(crate) fn append_buffer(pic: &PocketIc, content: &str, count: u64) {
            pic.update_call(
                active_canister(),
                Principal::anonymous(),
                "append_buffer",
                candid::encode_args((content.to_string(), count)).unwrap(),
            )
            .unwrap();
        }

        pub(crate) fn store_into_memory(pic: &PocketIc) -> (u64, u64) {
            let response = pic
                .update_call(
                    active_canister(),
                    Principal::anonymous(),
                    "store_into_memory",
                    candid::encode_one(0u64).unwrap(),
                )
                .unwrap();

            candid::decode_args(&response).unwrap()
        }
    }

    #[test]
    fn write_memory_test() {
        let pic = setup_initial_canister();

        fns::append_buffer(&pic, "abc1234567", 10_000_000);

        let (time, _size) = fns::store_into_memory(&pic);

        //panic!("Time elapsed: {time}");
    }

    /*
    #[test]
    fn check_iterator_with_insertions() {
        TEST_MAP.with(|m| {
            let mut m = m.borrow_mut();

            m.insert(1, "A".to_string());
            m.insert(2, "B".to_string());
            m.insert(3, "C".to_string());
            m.insert(6, "F".to_string());
            m.insert(7, "G".to_string());
            m.insert(8, "H".to_string());
            m.insert(4, "D".to_string());
            m.insert(5, "E".to_string());

            let mut it = m.range(2..7);

            let n = it.next().unwrap();
            println!("{} -> {}", n.0, n.1);
            let n = it.next().unwrap();
            println!("{} -> {}", n.0, n.1);
            let n = it.next().unwrap();
            println!("{} -> {}", n.0, n.1);
            let n = it.next().unwrap();
            println!("{} -> {}", n.0, n.1);
        });
    }*/
}

#[cfg(feature = "canbench-rs")]
mod benches {
    use crate::store_buffer;
    use canbench_rs::{bench, bench_fn, BenchResult};

    use crate::{append_buffer, store_buffer_4k, store_into_memory};

    #[bench(raw)]
    fn store_memory_bench() -> BenchResult {
        append_buffer("abc1234567".to_string(), 10_000_000);

        bench_fn(|| {
            store_into_memory(0);
        })
    }

    #[bench(raw)]
    fn store_buffer_bench() -> BenchResult {
        append_buffer("abc1234567".to_string(), 10_000_000);

        bench_fn(|| {
            store_buffer(1);
        })
    }

    #[bench(raw)]
    fn store_chunks_4k_bench() -> BenchResult {
        append_buffer("abc1234567".to_string(), 10_000_000);

        bench_fn(|| {
            store_buffer_4k(1);
        })
    }
}

export_candid!();
