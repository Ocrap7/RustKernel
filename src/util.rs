#![allow(dead_code)]

#[no_mangle]
#[inline(always)]
pub unsafe fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    asm!("rep movsb",
        inout("rcx") n => _,
        inout("rdi") dst => _,
        inout("rsi") src => _
    );
    dst
}

#[no_mangle]
#[inline(always)]
pub unsafe fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let v1 = *a.offset(i as isize);
        let v2 = *b.offset(i as isize);
        if v1 != v2 {
            return (v1 as i32).wrapping_sub(v2 as i32);
        }
    }
    0
}

#[no_mangle]
#[inline(always)]
pub unsafe fn memmove(dst: *mut u8, src: *const u8, mut n: usize) -> *mut u8 {
    if (dst as usize) > (src as usize) && (src as usize).wrapping_add(n) > (dst as usize) {
        let overhang = dst as usize - src as usize;

        if overhang < 64 {
            while n != 0 && (dst as usize).wrapping_add(n) & 0x7 != 0 {
                n = n.wrapping_sub(1);
                *dst.offset(n as isize) = *src.offset(n as isize);
            }

            while n >= 8 {
                n = n.wrapping_sub(8);

                let val = core::ptr::read_unaligned(src.offset(n as isize) as *const u64);

                core::ptr::write(dst.offset(n as isize) as *mut u64, val);
            }

            while n != 0 {
                n = n.wrapping_sub(1);
                *dst.offset(n as isize) = *src.offset(n as isize);
            }

            return dst;
        }

        while n >= overhang {
            n = n.wrapping_sub(overhang);

            let src = src.offset(n as isize);
            let dst = dst.offset(n as isize);

            memcpy(dst, src, overhang);
        }

        if n == 0 {
            return dst;
        }
    }
    memcpy(dst, src, n);
    dst
}

#[no_mangle]
#[inline(always)]
pub unsafe fn memset(ptr: *mut u8, value: i32, n: usize) -> *mut u8 {
    asm!("rep stosb",

        inout("rcx") n => _,
        inout("rdi") ptr => _,
        in("eax") value as u32
    );
    ptr
}

#[inline(always)]
pub unsafe fn out8(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value);
}

#[inline(always)]
pub unsafe fn out16(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value);
}

#[inline(always)]
pub unsafe fn out32(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value);
}

#[inline(always)]
pub unsafe fn in8(port: u16) -> u8 {
    let mut ret: u8;
    asm!("in al, dx", in("dx") port, out("al") ret);
    ret
}

#[inline(always)]
pub unsafe fn in16(port: u16) -> u16 {
    let mut ret: u16;
    asm!("in ax, dx", in("dx") port, out("ax") ret);
    ret
}

#[inline(always)]
pub unsafe fn in32(port: u16) -> u32 {
    let mut ret: u32;
    asm!("in eax, dx", in("dx") port, out("eax") ret);
    ret
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ({
        use core::fmt;
        let mut serial = $crate::drivers::serial::SerialPort::from(0x3F8);
        fmt::write(&mut serial, format_args!($($arg)*)).expect("Unable to print!");
    })
}

#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\r\n"));
    ($($arg:tt)*) => ({
        use core::fmt;
        let mut serial = $crate::drivers::serial::SerialPort::from(0x3F8);
        fmt::write(&mut serial, format_args!($($arg)*)).expect("Unable to print!");
        fmt::write(&mut serial, format_args!("\r\n")).expect("Unable to print!");
    })
}

#[derive(Debug, Default)]
pub struct CpuState {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsp: u64,
    rbp: u64,
    rsi: u64,
    rdi: u64,

    rip: u64,

    flags: u64
}