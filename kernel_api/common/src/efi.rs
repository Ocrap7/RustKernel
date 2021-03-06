use core::{fmt::Debug, ptr::null, sync::atomic::AtomicPtr};

use crate::{kprint, kprintln};

pub type Char16 = u16;
pub type Handle = usize;

const EMPTY_HANDLE: Handle = 0;

const BUFFER_TOO_SMALL: usize = 5 | (1 << 63);

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    size: u32,
    crc: u32,
    res: u32,
}

#[repr(C)]
pub struct SystemTable {
    header: TableHeader,
    vendor: *const Char16,
    revision: u32,
    console_in_handle: Handle,
    console_in: *const u8,
    console_out_handle: Handle,
    console_out: Handle,
    console_error_handle: Handle,
    console_error: *const u8,
    runtime_services: *const RuntimeServices,
    boot_services: *const BootServices,
    entry_count: usize,
    configuration_table: *mut ConfigurationTable,
}

impl SystemTable {
    pub fn config_tables(&self) -> ConfigurationTableIterator {
        ConfigurationTableIterator::new(self.configuration_table, self.entry_count)
    }

    pub fn boot_services(&self) -> &BootServices {
        unsafe { &*self.boot_services }
    }

    pub fn runtime_services(&self) -> &RuntimeServices {
        unsafe { &*self.runtime_services }
    }
}

#[repr(C)]
struct ConfigurationTable {
    guid: guid::GUID,
    ptr: *mut (),
}

pub struct ConfigurationTableIterator {
    configuration_base: *mut ConfigurationTable,
    size: usize,
    index: usize,
}

impl ConfigurationTableIterator {
    fn new(configuration_base: *mut ConfigurationTable, size: usize) -> ConfigurationTableIterator {
        ConfigurationTableIterator {
            configuration_base,
            size,
            index: 0,
        }
    }
}

impl Iterator for ConfigurationTableIterator {
    type Item = (&'static guid::GUID, *mut ());

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let ret = unsafe {
                let table = &*self.configuration_base.offset(self.index as _);
                Some((&table.guid, table.ptr))
            };
            self.index += 1;
            ret
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct RuntimeServices {
    header: TableHeader,

    /*
    Time services
    */
    get_time: Handle,
    set_time: Handle,
    get_wakeup_time: Handle,
    set_wakeup_time: Handle,

    /*
    Virtual Memory services
    */
    set_virtual_address_map:
        extern "efiapi" fn(usize, usize, u32, *const MemoryDescriptor) -> usize,
    convert_pointer: extern "efiapi" fn() -> usize,
}

impl RuntimeServices {
    pub fn set_virtual_address_map(&self, map: MemoryMap<'_>, version: u32) -> usize {
        let map_size = core::mem::size_of_val(map);
        let entry_size = core::mem::size_of::<MemoryDescriptor>();
        let map_ptr = map.as_ptr();
        (self.set_virtual_address_map)(map_size, entry_size, version, map_ptr)
    }
}

#[repr(C)]
pub struct BootServices {
    header: TableHeader,

    /*
    Task Priority Services
    */
    raise_tpl: Handle,
    restore_tple: Handle,

    /*
    Memory Services
     */
    allocate_pages: Handle,
    free_pages: Handle,
    get_memory_map:
        extern "efiapi" fn(&mut usize, *mut u8, &mut usize, &mut usize, &mut u32) -> usize,
    // extern "efiapi" fn(&mut usize, &mut [MemoryDescriptor], &mut usize, &mut usize, &mut u32) -> usize,
    allocate_pool: extern "efiapi" fn(MemoryType, usize, *mut *mut ()) -> usize,
    free_pool: extern "efiapi" fn(*mut ()) -> usize,

    /*
    Event & Timer Services
     */
    create_event: Handle,
    set_timer: Handle,
    wait_for_event: Handle,
    signal_event: Handle,
    close_event: Handle,
    check_event: Handle,

    /*
    Protocol Handler Services
     */
    install_protocol_interface: Handle,
    reinstall_protocol_interface: Handle,
    uninstall_protocol_interface: Handle,
    handle_protocol: extern "efiapi" fn(Handle, *const guid::GUID, *mut *const ()) -> usize,
    reserved: usize,
    register_protocol_notify: Handle,
    locate_handle: Handle,
    locate_device_path: Handle,
    install_configuration_table: Handle,

    /*
    Image services
     */
    image_load: extern "efiapi" fn(),
    start_image: Handle,
    exit: Handle,
    image_unload: Handle,
    exit_boot_services: extern "efiapi" fn(Handle, usize) -> usize,

    /*
    Miscellaneaous Services
    */
    get_next_monotonic_count: Handle,
    stall: Handle,
    set_watchdog_timer: extern "efiapi" fn(usize, u64, usize, *const Char16) -> usize,

    /*
    Driver Support Services
    */
    connect_controller: Handle,
    disconnect_controller: Handle,

    open_protocol:
        extern "efiapi" fn(Handle, *const guid::GUID, *mut *const (), Handle, Handle, u32) -> usize,
    close_protocol: Handle,
    open_protocol_info: Handle,

    protocols_per_handle: Handle,
    locate_handle_buffer: Handle,
    locate_protocol: extern "efiapi" fn(*const guid::GUID, *const (), *mut *const ()) -> usize,
}

const OPEN_PROTOCOL_BY_HANDLE_PROTOCOL: u32 = 0x01;

impl BootServices {
    fn handle_protocol<T>(
        &self,
        handle: Handle,
        guid: &guid::GUID,
        protocol: &mut *const T,
    ) -> usize {
        unsafe {
            let ptr = protocol as *mut *const T;
            (self.handle_protocol)(handle, guid, ptr as *mut *const ())
        }
    }

    fn open_protocol<T>(
        &self,
        handle: Handle,
        protocol: &guid::GUID,
        interface: &mut *const T,
        agent_handle: Handle,
        controller_handle: Handle,
        attributes: u32,
    ) -> usize {
        unsafe {
            let ptr = interface as *mut *const T;
            (self.open_protocol)(
                handle,
                protocol,
                ptr as *mut *const (),
                agent_handle,
                controller_handle,
                attributes,
            )
        }
    }

    pub fn allocate_pool<T>(&self, size: usize, ptr: &mut *mut T) -> usize {
        let ptr = ptr as *mut *mut T;
        (self.allocate_pool)(
            MemoryType::LoaderData,
            size * core::mem::size_of::<T>(),
            ptr as *mut *mut (),
        )
    }

    pub fn free_pool<T: ?Sized>(&self, ptr: &mut T) -> usize {
        let ptr = ptr as *mut T;
        (self.free_pool)(ptr as *mut ())
    }

    pub fn set_watchdog_timer(&self, timeout: usize, watchdog_code: u64) -> usize {
        (self.set_watchdog_timer)(timeout, watchdog_code, 0, core::ptr::null())
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct LoadedImage {
    revision: u32,
    parent_handle: Handle,
    system_table: *const SystemTable,

    device_handle: Handle,
    file_path: Handle,
    reserved: *const (),

    load_options_size: u32,
    load_options: *const (),

    image_base: *const (),
    image_size: usize,
    image_code_type: MemoryType,
    image_data_type: MemoryType,
    unload: Handle,
}

#[repr(C, packed)]
pub struct FileIOInterface {
    revision: u64,
    pub open_volume: extern "efiapi" fn(*const FileIOInterface, *mut *const FileProtocol) -> usize,
}

#[repr(C)]
pub struct FileProtocol {
    revision: u64,
    pub open: extern "efiapi" fn(
        *const FileProtocol,
        *mut *const FileProtocol,
        *const Char16,
        u64,
        u64,
    ) -> usize,
    pub close: extern "efiapi" fn(*const FileProtocol) -> usize,
    pub delete: extern "efiapi" fn(*const FileProtocol) -> usize,
    pub read: extern "efiapi" fn(*const FileProtocol, *mut usize, *mut u8) -> usize,
    pub write: extern "efiapi" fn(*const FileProtocol) -> usize,
    pub get_position: extern "efiapi" fn(*const FileProtocol) -> usize,
    pub set_position: extern "efiapi" fn(*const FileProtocol, usize) -> usize,
    pub get_info:
        extern "efiapi" fn(*const FileProtocol, *const guid::GUID, *mut usize, *mut FileInfo) -> usize,
}

#[repr(C, packed)]
#[derive(Debug, Default)]
pub struct Time {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    pad1: u8,
    nanosecond: u32,
    time_zone: i16,
    daylight: u8,
    pad2: u8,
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct FileInfo {
    pub size: usize,
    pub file_size: usize,
    pub physical_size: usize,
    pub create_time: Time,
    pub last_access_time: Time,
    pub modification_time: Time,
    pub attribute: u64,
    pub file_name: [u16; 10],
}

impl Default for FileInfo {
    fn default() -> Self {
        FileInfo {
            file_name: [0; 10],
            ..Default::default()
        }
    }
}

pub fn io_volume(image_handle: Handle) -> *const FileIOInterface {
    let table = get_system_table();

    let mut loaded_image: *const LoadedImage = core::ptr::null();
    let mut io_volume: *const FileIOInterface = core::ptr::null();
    let mut file: *const FileProtocol = core::ptr::null();
    unsafe {
        let res = table.boot_services().open_protocol(
            image_handle,
            &guid::LOADED_IMAGE_PROTOCOL,
            &mut loaded_image,
            image_handle,
            EMPTY_HANDLE,
            OPEN_PROTOCOL_BY_HANDLE_PROTOCOL,
        );
        if res != 0 {
            kprintln!("An error occured! {:x} HandleProtocol(LIP)", res);
        }

        kprintln!("{:x?}", *loaded_image);

        let res = table.boot_services().open_protocol(
            (*loaded_image).device_handle,
            &guid::SIMPLE_FILE_SYSTEM_PROTOCOL,
            &mut io_volume,
            image_handle,
            EMPTY_HANDLE,
            OPEN_PROTOCOL_BY_HANDLE_PROTOCOL,
        );

        if res != 0 {
            kprintln!("An error occured! {:x} HandleProtocol(SFSP)", res);
        }
        io_volume
    }
}

pub fn read_fixed(file: &FileProtocol, offset: usize, size: usize, buffer: &mut [u8]) -> usize {
    let mut read = 0usize;

    // let status = (file.set_position)(file, offset + read);
    // if status != 0 {
    //     kprintln!("An error occured! {:x} SETPOSTIOIN(SFSP)", status);
    //     return status;
    // }

    // while read < size {
    let mut remain = buffer.len();

    (file.read)(file, &mut remain, buffer.as_mut_ptr())
    // if status != 0 {
    //     kprintln!(
    //         "An error occured! {:x} READ(SFSP) {} {} {:p}",
    //         status,
    //         remain,
    //         read,
    //         &mut buffer[read] as *mut _ as *mut () // buffer
    //     );
    //     // return status;
    // }

    //     read += remain;
    // }

    // 0
}

pub const FILE_MODE_READ: u64 = 1;
pub const FILE_READ_ONLY: u64 = 1;
pub const FILE_HIDDEN: u64 = 2;
pub const FILE_SYSTEM: u64 = 4;

#[repr(C, packed)]
pub struct FileHandle {}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq)]
pub enum MemoryType {
    Reserved,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    Conventional,
    Unusable,
    ACPIReclaim,
    ACPINVS,
    MemoryMappedIO,
    MemoryMappedIOPortSpace,
    PalCode,
    PersistentMemory,
    MaxMemoryType,
}

impl MemoryType {
    pub fn is_usable(&self) -> bool {
        match self {
            Self::BootServicesCode
            // | Self::BootServicesData 
            // | Self::PersistentMemory,
            | Self::Conventional => true,
            _ => false,
        }
    }
}

impl Debug for MemoryType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self as u32 {
            0 => write!(f, "Reserved"),
            1 => write!(f, "LoaderCode"),
            2 => write!(f, "LoaderData"),
            3 => write!(f, "BootServicesCode"),
            4 => write!(f, "BootServicesData"),
            5 => write!(f, "RuntimeServicesCode"),
            6 => write!(f, "RuntimeServicesData"),
            7 => write!(f, "Conventional"),
            8 => write!(f, "Unusable"),
            9 => write!(f, "ACPIReclaim"),
            10 => write!(f, "ACPINVS"),
            11 => write!(f, "MemoryMappedIO"),
            12 => write!(f, "MemoryMappedIOPortSpace"),
            13 => write!(f, "PalCode"),
            14 => write!(f, "PersistentMemory"),
            15 => write!(f, "MaxMemoryType"),
            _ => write!(f, "Unknown Memory Type"),
        }
    }
}

impl MemoryType {
    fn as_u8(&self) -> u32 {
        *self as u32
    }
}

impl Default for MemoryType {
    fn default() -> Self {
        MemoryType::Reserved
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryDescriptor {
    pub memory_type: MemoryType,
    pub physical_address: usize,
    pub virtual_address: usize,
    pub size: usize,
    pub attributes: u64,
    pub r1: u64,
    // r2: u32,
}

impl MemoryDescriptor {
    pub fn is_runtime(&self) -> bool {
        self.attributes & 0x8000000000000000 > 0
    }
}

pub type MemoryMap<'a> = &'a [MemoryDescriptor];

// #[repr(C)]
// pub struct SimpleTextOutputProtocol {
//     reset: extern "efiapi" fn(*mut Self),
//     output_string: extern "efiapi" fn(*mut Self, *const u16),
//     test_string: extern "efiapi" fn(&Self),
//     query_mode: extern "efiapi" fn(&Self),
//     set_mode: extern "efiapi" fn(&Self),
//     set_attribute: extern "efiapi" fn(&Self),
//     clear_screen: extern "efiapi" fn(&Self),
//     set_cursor_position: extern "efiapi" fn(&Self),
//     enable_cursor: extern "efiapi" fn(&Self),
//     mode: *const u8,
// }

pub static GLOBAL_SYSTEM_TABLE: AtomicPtr<SystemTable> = AtomicPtr::new(core::ptr::null_mut());

pub unsafe fn register_global_system_table(
    table: *mut SystemTable,
) -> Result<*mut SystemTable, *mut SystemTable> {
    GLOBAL_SYSTEM_TABLE.compare_exchange(
        core::ptr::null_mut(),
        table,
        core::sync::atomic::Ordering::SeqCst,
        core::sync::atomic::Ordering::SeqCst,
    )
}

// pub fn output(string: &str) {
//     let buff = ['a' as char16 ; 5];
//     let table = GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst);

//     if table.is_null() {
//         return;
//     }

//     let out = unsafe { (*table).console_out };

//     unsafe {
//         ((*out).output_string)(out, buff.as_ptr());
//     }
// }
pub static mut DESCRIPTORS: [MemoryDescriptor; 1024] = [MemoryDescriptor {
    attributes: 0,
    memory_type: MemoryType::Reserved,
    physical_address: 0,
    r1: 0,
    size: 0,
    virtual_address: 0,
}; 1024];

pub fn get_memory_map(image_handle: Handle) -> (MemoryMap<'static>, u32) {
    let table = GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst);

    unsafe {
        let mut size = core::mem::size_of_val(&DESCRIPTORS);
        let mut key = 0;
        let mut mdesc_size = 0;
        let mut mdesc_version = 0;

        let result = ((*(*table).boot_services).get_memory_map)(
            &mut size,
            DESCRIPTORS.as_mut_ptr() as *mut u8,
            &mut key,
            &mut mdesc_size,
            &mut mdesc_version,
        );

        assert!(result == 0, " {:x?} {:x}", result, BUFFER_TOO_SMALL);

        // print_memory_map(&DESCRIPTORS);

        let result = ((*(*table).boot_services).exit_boot_services)(image_handle, key);
        assert!(result == 0, "Unable to exit boot services! {:x}", result);
        kprintln!("Exited boot services!");
        return (&DESCRIPTORS, mdesc_version);
    }
}

pub fn print_memory_map(map: MemoryMap<'_>) {
    let mut conventional = 0;
    let mut all = 0;
    for desc in map {
        if desc.physical_address == 0 && desc.virtual_address == 0 && desc.size == 0 {
            break;
        }

        all += desc.size * 4096;
        // if desc.memory_type.is_usable() {
        // }
        if let MemoryType::Conventional = desc.memory_type {
            conventional += desc.size * 4096;
        }

        kprintln!(
            "{:016x} {:016x} {:016x} {:?} Runtime {}",
            desc.physical_address,
            desc.virtual_address,
            desc.size * 4096,
            desc.memory_type,
            desc.attributes & 0x8000000000000000 > 0
        );
    }
    kprintln!("all: {:x?}, conv: {:x}", all, conventional);
}

pub fn get_mem_size(map: MemoryMap<'_>) -> usize {
    let mut all = 0;
    for desc in map {
        if desc.physical_address == 0 && desc.virtual_address == 0 && desc.size == 0 {
            break;
        }
        all += desc.size * 4096;
    }
    all
}

pub fn get_image_base(image_handle: Handle) -> usize {
    let table = GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst);

    let mut loaded_image: *const LoadedImage = core::ptr::null();
    unsafe {
        let res = (*(*table).boot_services).handle_protocol(
            image_handle,
            &guid::LOADED_IMAGE_PROTOCOL,
            &mut loaded_image,
        );
        if res != 0 {
            kprintln!("An error occured! {:x}", res);
        }
        kprintln!("{:p}", loaded_image);
        (*loaded_image).image_base as _
    }
}

pub fn get_system_table() -> &'static SystemTable {
    unsafe { &*GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst) }
}

pub mod guid {

    use core::fmt::Display;

    use alloc::fmt::format;
    pub use macros::create_guid;

    #[derive(PartialEq)]
    pub struct GUID {
        a: u32,
        /// The middle field of the timestamp.
        b: u16,
        /// The high field of the timestamp multiplexed with the version number.
        c: u16,
        /// Contains, in this order:
        /// - The high field of the clock sequence multiplexed with the variant.
        /// - The low field of the clock sequence.
        /// - The spatially unique node identifier.
        d: [u8; 8],
    }

    impl Display for GUID {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                f,
                "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                self.a,
                self.b,
                self.c,
                self.d[0],
                self.d[1],
                self.d[2],
                self.d[3],
                self.d[4],
                self.d[5],
                self.d[6],
                self.d[7]
            )
            // let dstr = self.d.iter().skip(2).map(|f| format(format_args!("{:x}", f)));
        }
    }

    impl<'a> PartialEq<GUID> for &'a GUID {
        fn eq(&self, other: &GUID) -> bool {
            self.a == other.a && self.b == other.b && self.c == other.c && self.d == other.d
        }
    }

    pub const LOADED_IMAGE_PROTOCOL: GUID = create_guid!(5B1B31A1-9562-11d2-8E3F-00A0C969723B);

    pub const RAM_DISK_PROTOCOL: GUID = create_guid!(ab38a0df-6873-44a9-87e6-d4eb56148449);

    pub const SIMPLE_FILE_SYSTEM_PROTOCOL: GUID =
        create_guid!(964e5b22-6459-11d2-8e39-00a0c969723b);

    pub const RSDP: GUID = create_guid!(8868E871-E4F1-11D3-BC22-0080C73C8881);

    pub const FILE_INFO: GUID = create_guid!(09576e92-6d3f-11d2-8e39-00a0c969723b);
}
