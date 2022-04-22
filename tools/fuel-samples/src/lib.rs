#![allow(dead_code)]


use num::integer::Integer;

use lazy_static::lazy_static;
use primes::PrimeSet;
use rand::seq::SliceRandom;
use rand::{RngCore, SeedableRng};


macro_rules! repeat4 {
    ($e:expr) => { {$e; $e; $e; $e;} }
}

macro_rules! repeat16 {
    ($e:expr) => { {$e; $e; $e; $e; $e; $e; $e; $e; $e; $e; $e; $e; $e; $e; $e; $e;} }
}

const MODULES: [(&str, fn()); 12] = [
    ("cmp5", cmp::<512>),
    ("cmp10", cmp::<1024>),
    ("cmp16", cmp::<65536>),
    ("cmp20", cmp::<1048576>),
    ("simple_add", simple_add),
    ("sieve", sieve),
    ("div_rem", div_rem),
    ("syscall", syscall),
    ("stack_alloc", stack_alloc),
    ("heap_alloc", heap_alloc),
    ("small_pointer_chase", small_pointer_chase),
    ("large_pointer_chase", large_pointer_chase)
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
    static ref LESS_WORST_LINKED_LIST: Vec<u64> = {
        let mut wll : Vec<u64> = (0..(1<<10)).collect();

        let mut rng = rand_xorshift::XorShiftRng::from_seed([42u8; 16]);
        wll.as_mut_slice().shuffle(&mut rng);
        wll
    };
}

#[no_mangle]
pub extern "C" fn init() {
    black_box(WORST_LINKED_LIST[1]);
    black_box(LESS_WORST_LINKED_LIST[1]);
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

        repeat16!{
            i += k1
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
        repeat16!{black_box(1)};
    }
}

fn stack_alloc() {
    loop {
        let x = [1; 1<<16];
        black_box(&x);
    }
}

fn heap_alloc() {
    loop {
        let x = vec![1; 1<<20];
        black_box(x.as_slice());
    }
}


fn div_rem() {
    let mut i = black_box(1u64<<63-1);
    let mut rem = 0u64;
    let k = black_box(7u64);
    loop {
        repeat4!({repeat16!({
            let (i2, rem2) = i.div_rem(&k);
            i = i2;
            rem += rem2;
        })});

        black_box(i);
        black_box(rem);
        i = black_box(1u64<<63-1);
    }

}

fn cmp<const N: usize>() {
    let mut rng = rand_xorshift::XorShiftRng::from_seed([42u8; 16]);
    let mut v1 = vec![0u8; N];
    rng.fill_bytes(&mut v1);

    let mut v2 = vec![0u8; N];
    rng.fill_bytes(&mut v2);

    loop {
        let eq = v1 == v2;
        if eq {
            black_box_mut(&mut v1);
        } else {
            black_box_mut(&mut v2);
        }
    }

}

fn small_pointer_chase() {
    pointer_chase(&LESS_WORST_LINKED_LIST);
}

fn large_pointer_chase() {
    pointer_chase(&WORST_LINKED_LIST);
}

fn pointer_chase(wll: &Vec<u64>) {
    let mut i = black_box(0_usize);
    loop {
        let last = i;
        repeat16!{
            repeat4!{i = wll[i] as usize}
        };
        if last == i {
            break;
        }
    }
    black_box(i);
}
