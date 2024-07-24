#!ic-repl


function install(wasm, args, cycle) {
  let id = call ic.provisional_create_canister_with_cycles(record { settings = null; amount = cycle });
  let S = id.canister_id;
  let _ = call ic.install_code(
    record {
      arg = args;
      wasm_module = gzip(wasm);
      mode = variant { install };
      canister_id = S;
    }
  );
  S
};

function upgrade(id, wasm, args) {
  call ic.install_code(
    record {
      arg = args;
      wasm_module = gzip(wasm);
      mode = variant { upgrade };
      canister_id = id;
    }
  );
};

function uninstall(id) {
  call ic.stop_canister(record { canister_id = id });
  call ic.delete_canister(record { canister_id = id });
};

function get_memory(cid) {
  let _ = call ic.canister_status(record { canister_id = cid });
  _.memory_size
};

let file = "README.md";

let rs_config = record { start_page = 1; page_limit = 128};

let wasm_name = "target/wasm32-unknown-unknown/release/treemap_chunks_backend.wasm";


function store_chunk() {

  let cid = install(wasm_profiling(wasm_name, rs_config), encode (), null);

  // turn off tracing
  call cid.__toggle_tracing();

  call cid.append_chunk( "abcdef7890", (10000000: nat64) );


  // turn on tracing
  call cid.__toggle_tracing();

  call cid.store_chunk( (10: nat64) );
  flamegraph(cid, "store_chunk", "svg/store_chunk.svg");
  uninstall(cid);
};

function store_chunk_4k() {

  let cid = install(wasm_profiling(wasm_name, rs_config), encode (), null);

  // turn off tracing
  call cid.__toggle_tracing();

  call cid.append_chunk( "abcdef7890", (10000000: nat64) );

  call cid.store_chunk_4k( (10: nat64) );

  call cid.append_chunk( "abcdef7890", (1000: nat64) );

  // turn on tracing
  call cid.__toggle_tracing();

  call cid.store_chunk_4k( (11: nat64) );

  flamegraph(cid, "store_chunk_4k", "svg/store_chunk_4k.svg");

  uninstall(cid);
};

function store_chunk_4k_small() {

  let cid = install(wasm_profiling(wasm_name, rs_config), encode (), null);

  // turn off tracing
  //call cid.__toggle_tracing();

  //call cid.append_chunk( "abcdef7890", (10000000: nat64) );

  //call cid.store_chunk_4k( (10: nat64) );

  call cid.append_chunk( "abcdef7890", (10000: nat64) );

  // turn on tracing
  //call cid.__toggle_tracing();

  call cid.store_chunk_4k( (11: nat64) );

  flamegraph(cid, "store_chunk_4k_small", "svg/store_chunk_4k_small.svg");

  uninstall(cid);
};



/// files
//store_chunk();
store_chunk_4k();
