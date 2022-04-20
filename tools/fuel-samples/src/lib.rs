#![allow(dead_code)]


use std::ops::Deref;

use lazy_static::lazy_static;
use primes::PrimeSet;
use rand::seq::SliceRandom;
use rand::SeedableRng;

const MODULES: [(&str, fn()); 4] = [
    ("simple_add", simple_add),
    ("sieve", sieve),
    ("syscall", syscall),
    ("pointer_chase", pointer_chase),
];

extern "C" {
    fn black_box1(ptr: *mut u8);
    fn black_box2(ptr: *const u8) -> *const u8;
}


fn black_box_mut<T>(x: &mut T) {
    unsafe {
        black_box1((x as *mut T) as *mut u8);
    }
}


fn black_box<T: Copy>(x: T) -> T {
    unsafe {
        *(black_box2((&x as *const T) as *mut u8) as *const T)
    }
}

pub fn getrandom_dummy(dest: &mut [u8]) -> Result<(), getrandom::Error> {
    dest[0] = 42;
    Ok(())
}
getrandom::register_custom_getrandom!(getrandom_dummy);

lazy_static! {
    static ref WORST_LINKED_LIST: Vec<u64> = {
        let mut wll : Vec<u64> = (0..(1<<23)).collect();

        let mut rng = rand_xorshift::XorShiftRng::from_seed([42u8; 16]);
        wll.as_mut_slice().shuffle(&mut rng);
        wll
    };
}

#[no_mangle]
pub extern "C" fn init() {
    black_box(WORST_LINKED_LIST[1]);
}

#[no_mangle]
pub extern "C" fn list() -> *const u8 {
    let mut joined = MODULES.map(|a| a.0).join("\n");
    joined.push('\0');
    joined.as_ptr()
}

#[no_mangle]
pub extern "C" fn invoke(i: u32) {
    MODULES[i as usize].1();
}

fn simple_add() {
    let mut i = black_box(7u64);
    let k1 = black_box(27u64);

    loop {
        let last = i;

        for _ in 0..16 {
            i += k1;
        }

        if i ^ last == k1 {
            break;
        }
    }
    black_box(i);
}

fn sieve() {
    let mut total = 0u64;
    for p in primes::Sieve::new().iter() {
        total += p;

        if total < 1 {
            break;
        }
    }
    black_box(total);
}

fn syscall() {
    loop {
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
        black_box(1);
    }
}

// stack allocs

// heap allocs

// div

// binary cmp

fn small_pointr_chase() {}

fn pointer_chase() {
    let mut i = black_box(0_usize);
    let wll = WORST_LINKED_LIST.deref();
    loop {
        let last = i;

        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        i = wll[i] as usize;
        if last == i {
            break;
        }
    }
    black_box(i);
}
