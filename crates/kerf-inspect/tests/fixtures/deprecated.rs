// Calls to `old_api` are deprecated — `new_api` replaces it.

fn old_api(x: u32) -> u32 {
    x + 1
}

fn new_api(x: u32) -> u32 {
    x + 1
}

fn main() {
    let a = old_api(1);
    let b = old_api(a);
    println!("{}", new_api(b));
}
