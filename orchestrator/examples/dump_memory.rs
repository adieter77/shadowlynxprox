// Quick debug: load the plugin, call a tool, dump raw output bytes
use wasmtime::*;

fn main() {
    let wasm_path = std::env::args().nth(1).unwrap_or_else(|| {
        "/home/shadowlynxupgraded/shadowlynx-prox/plugins/offensive/offensive.wasm".to_string()
    });
    let tool_name = std::env::args().nth(2).unwrap_or_else(|| "ping".to_string());
    let arg = std::env::args().nth(3).unwrap_or_else(|| "127.0.0.1".to_string());

    let wasm_bytes = std::fs::read(&wasm_path).expect("read wasm");
    eprintln!("WASM file: {} ({} bytes)", wasm_path, wasm_bytes.len());

    let mut config = Config::new();
    config.consume_fuel(true);
    config.wasm_multi_memory(true);
    config.wasm_bulk_memory(true);

    let engine = Engine::new(&config).unwrap();
    let module = Module::from_binary(&engine, &wasm_bytes).unwrap();

    let mut store: Store<()> = Store::new(&engine, ());
    store.set_fuel(2_000_000_000).ok();

    let mut linker: Linker<()> = Linker::new(&engine);

    linker.func_wrap("slpx", "log", |_a: i32, _b: i32, _c: i32| {}).unwrap();
    linker.func_wrap("slpx", "http_get", |_a: i32, _b: i32, _c: i32| 0i32).unwrap();
    linker.func_wrap("slpx", "get_time", || -> i64 { 1752000000000i64 }).unwrap();
    linker.func_wrap("slpx", "read_file", |_a: i32, _b: i32, _c: i32| 0i32).unwrap();
    linker.func_wrap("slpx", "write_file", |_a: i32, _b: i32, _c: i32, _d: i32| 0i32).unwrap();
    linker.func_wrap("slpx", "random_bytes", |_a: i32, _b: i32, _c: i32| {}).unwrap();

    let instance = linker.instantiate(&mut store, &module).unwrap();
    let mem = instance.get_memory(&mut store, "memory").unwrap();

    {
        let data = mem.data(&store);
        for off in (1024..1875).step_by(32) {
            let end = (off + 32).min(data.len()).min(1875);
            if off >= data.len() { break; }
            let chunk = &data[off..end];
            let s: String = chunk.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
            eprintln!("{:5} | {} | {}", off, chunk.iter().map(|b| format!("{:02x}", b)).collect::<String>(), s);
        }
    }

    let alloc = instance.get_func(&mut store, "alloc").unwrap();
    let arg_bytes = arg.as_bytes();

    // Check heap global directly
    if let Some(heap_g) = instance.get_global(&mut store, "heap") {
        eprintln!("\nheap global value: {:?}", heap_g.get(&mut store));
    }

    // First, call alloc(0) just to see heap state
    let mut alloc_results = [Val::I32(0)];
    alloc.call(&mut store, &[Val::I32(0)], &mut alloc_results).unwrap();
    let p0 = match alloc_results[0] { Val::I32(v) => v, _ => 0 };
    eprintln!("alloc(0) returned: {}", p0);

    let mut alloc_results = [Val::I32(0)];
    alloc.call(&mut store, &[Val::I32(arg_bytes.len() as i32)], &mut alloc_results).unwrap();
    let arg_ptr = match alloc_results[0] { Val::I32(v) => v, _ => 0 };
    eprintln!("alloc({}) returned: {}", arg_bytes.len(), arg_ptr);

    {
        let data = mem.data_mut(&mut store);
        data[arg_ptr as usize..arg_ptr as usize + arg_bytes.len()].copy_from_slice(arg_bytes);
    }

    let func_name = format!("tool_{}", tool_name);
    let func = instance.get_func(&mut store, &func_name).unwrap();
    let mut results = [Val::I64(0)];
    func.call(&mut store, &[Val::I32(arg_ptr), Val::I32(arg_bytes.len() as i32)], &mut results).unwrap();

    let packed = match results[0] { Val::I64(v) => v, _ => 0 };
    let result_ptr = (packed >> 32) as i32;
    let result_len = (packed & 0xFFFF_FFFF) as i32;
    eprintln!("\nPacked: {:#018x} -> ptr={}, len={}", packed, result_ptr, result_len);

    if result_ptr >= 0 && result_len > 0 {
        let data = mem.data(&store);
        let s = result_ptr as usize;
        let e = s + result_len as usize;
        eprintln!("\n=== Output bytes [{:#x}..{:#x}] ===", s, e);
        for off in (s..e).step_by(32) {
            let end = (off + 32).min(e);
            let chunk = &data[off..end];
            let ascii: String = chunk.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
            eprintln!("{:5} | {} | {}", off, chunk.iter().map(|b| format!("{:02x}", b)).collect::<String>(), ascii);
        }
        let result_str = String::from_utf8_lossy(&data[s..e]);
        eprintln!("\nResult: {}", result_str);
    }
}


