#![no_std]
#![no_main]
#![allow(non_snake_case)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(asm)]
#![allow(unused)]

extern crate alloc;

mod drivers;
mod efi;
#[macro_use]
mod util;
mod allocator;
mod gdt;
mod interrupts;
mod mem;
mod processes;

use core::panic::PanicInfo;

// use alloc::boxed::Box;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::mapper::TranslateResult;
use x86_64::structures::paging::{OffsetPageTable, PageTable, PhysFrame, Translate};
use x86_64::{PhysAddr, VirtAddr};

use crate::processes::{test_process, Process};
// use crate::util::CpuState;

#[no_mangle]
extern "C" fn efi_main(image_handle: efi::Handle, system_table: *mut efi::SystemTable) {

    unsafe {
        // Set the static system table reference
        efi::register_global_system_table(system_table).unwrap();
    }

    let base = efi::get_image_base(image_handle);


    kprintln!("Entry: {:x}", base);
    let wait = true;
    while wait {
        unsafe {
            asm!("pause")
        }
    }

    // Iterate memorymap and exit boot services
    let memory_map = efi::get_memory_map(image_handle);

    // Setup global descriptor table :P
    gdt::init();

    // Setup interrupts
    interrupts::init();

    let mut mapper = unsafe { mem::init() };
    let mut frame_allocator = mem::PageTableFrameAllocator::new(memory_map);

    let mut npt = mapper.level_4_table().clone();
    let mut mapper = unsafe { OffsetPageTable::new(&mut npt, VirtAddr::new(0)) };
    let table: *mut PageTable = mapper.level_4_table();

    unsafe {
        Cr3::write(
            PhysFrame::from_start_address(PhysAddr::new(table as u64))
                .expect("Unable to switch page table!"),
            Cr3Flags::empty(),
        );
        mem::KERNEL_MAP = table as u64;
        kprintln!("KMAP {:x}", mem::KERNEL_MAP);
        // mem::KERNEL_MAP.store(table, core::sync::atomic::Ordering::SeqCst);
    }

    allocator::init_heap(&mut mapper, &mut frame_allocator, false).expect("Unable to create heap!");

    let addresses = [processes::test_process as u64]; // same as before

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let res = mapper.translate(virt);
        match res {
            TranslateResult::Mapped { frame, flags, .. } => {
                kprintln!("Frame: {:#x?} {:?}", frame, flags);
            }
            _ => (),
        }
    }

    let new_process = Process::new(test_process, &mut frame_allocator);

    processes::set_syscall_sp();
    unsafe {
        processes::jump_usermode(&mapper, &new_process);
    }

    kprintln!("Done!");

    loop {}
    // panic!("Kernel Finished");
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    kprintln!("PANIC! {}\n", _info);
    loop {}
}