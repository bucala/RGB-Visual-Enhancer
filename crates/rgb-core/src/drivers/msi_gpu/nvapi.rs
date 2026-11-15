//! Minimal NvAPI binding for GPU I2C access on Windows (`nvapi64.dll` /
//! `nvapi.dll`, shipped with the NVIDIA driver). The I2C entry points are
//! not in NVIDIA's public SDK; interface ids and struct layout follow
//! OpenRGB's `dependencies/NVFC/nvapi.*`.

use std::ffi::{c_void, CStr};
use std::io;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use super::GpuI2cBus;

type Handle = *mut c_void;
type QueryInterface = unsafe extern "C" fn(u32) -> *mut c_void;
type Initialize = unsafe extern "C" fn() -> i32;
type EnumPhysicalGpus = unsafe extern "C" fn(*mut Handle, *mut i32) -> i32;
type GetPciIdentifiers =
    unsafe extern "C" fn(Handle, *mut u32, *mut u32, *mut u32, *mut u32) -> i32;
type I2cTransfer = unsafe extern "C" fn(Handle, *mut I2cInfoV3, *mut u32) -> i32;

const ID_INITIALIZE: u32 = 0x0150_E828;
const ID_ENUM_PHYSICAL_GPUS: u32 = 0xE5AC_921F;
const ID_GET_PCI_IDENTIFIERS: u32 = 0x2DDF_B66E;
const ID_I2C_WRITE_EX: u32 = 0x283A_C65A;
const ID_I2C_READ_EX: u32 = 0x4D7B_0709;

const MAX_GPUS: usize = 64;
/// GPU I2C port that carries the RGB controller on MSI cards.
const RGB_PORT: u8 = 1;

/// `NV_I2C_INFO_V3`, default C alignment.
#[repr(C)]
struct I2cInfoV3 {
    version: u32,
    display_mask: u32,
    is_ddc_port: u8,
    i2c_dev_address: u8,
    i2c_reg_address: *mut u8,
    reg_addr_size: u32,
    data: *mut u8,
    size: u32,
    i2c_speed: u32,
    i2c_speed_khz: u32,
    port_id: u8,
    is_port_id_set: u32,
}

// C layout: 64 bytes on x64, 44 on x86 (the size is part of `version`).
const _: () = assert!(
    std::mem::size_of::<I2cInfoV3>()
        == if cfg!(target_pointer_width = "64") {
            64
        } else {
            44
        }
);

struct Api {
    enum_gpus: EnumPhysicalGpus,
    pci_ids: GetPciIdentifiers,
    write: I2cTransfer,
    read: I2cTransfer,
}

// Function pointers into a DLL that is never unloaded.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

static API: OnceLock<Option<Api>> = OnceLock::new();
/// NvAPI I2C calls are serialised process-wide.
static BUS_LOCK: Mutex<()> = Mutex::new(());

fn load() -> Option<Api> {
    let dll: &CStr = if cfg!(target_pointer_width = "64") {
        c"nvapi64.dll"
    } else {
        c"nvapi.dll"
    };
    // SAFETY: plain Win32 calls with NUL-terminated names; every pointer
    // obtained is null-checked before being transmuted to its signature.
    unsafe {
        let module = LoadLibraryA(dll.as_ptr().cast());
        if module.is_null() {
            return None; // no NVIDIA driver installed
        }
        let qi = GetProcAddress(module, c"nvapi_QueryInterface".as_ptr().cast())?;
        let qi: QueryInterface = std::mem::transmute(qi);
        let get = |id: u32| {
            let p = qi(id);
            (!p.is_null()).then_some(p)
        };
        let init: Initialize = std::mem::transmute(get(ID_INITIALIZE)?);
        if init() != 0 {
            return None;
        }
        Some(Api {
            enum_gpus: std::mem::transmute::<*mut c_void, EnumPhysicalGpus>(get(
                ID_ENUM_PHYSICAL_GPUS,
            )?),
            pci_ids: std::mem::transmute::<*mut c_void, GetPciIdentifiers>(get(
                ID_GET_PCI_IDENTIFIERS,
            )?),
            write: std::mem::transmute::<*mut c_void, I2cTransfer>(get(ID_I2C_WRITE_EX)?),
            read: std::mem::transmute::<*mut c_void, I2cTransfer>(get(ID_I2C_READ_EX)?),
        })
    }
}

fn api() -> Option<&'static Api> {
    API.get_or_init(load).as_ref()
}

/// PCI identity of one NVIDIA GPU.
pub struct GpuIds {
    handle: usize,
    pub vendor: u16,
    pub device: u16,
    pub subvendor: u16,
    pub subdevice: u16,
}

/// Enumerate physical NVIDIA GPUs. `None` when NvAPI is unavailable.
pub fn enumerate() -> Option<Vec<GpuIds>> {
    let api = api()?;
    let mut handles: [Handle; MAX_GPUS] = [std::ptr::null_mut(); MAX_GPUS];
    let mut count: i32 = 0;
    let _guard = BUS_LOCK.lock().ok()?;
    // SAFETY: buffers sized as NvAPI expects (64 handles).
    if unsafe { (api.enum_gpus)(handles.as_mut_ptr(), &mut count) } != 0 {
        return Some(Vec::new());
    }
    let count = (count.max(0) as usize).min(MAX_GPUS);
    Some(
        handles[..count]
            .iter()
            .filter_map(|&h| {
                let (mut dev, mut sub, mut rev, mut ext) = (0u32, 0u32, 0u32, 0u32);
                // SAFETY: valid handle from EnumPhysicalGPUs; out-pointers are locals.
                let rc = unsafe { (api.pci_ids)(h, &mut dev, &mut sub, &mut rev, &mut ext) };
                (rc == 0).then_some(GpuIds {
                    handle: h as usize,
                    vendor: (dev & 0xFFFF) as u16,
                    device: (dev >> 16) as u16,
                    subvendor: (sub & 0xFFFF) as u16,
                    subdevice: (sub >> 16) as u16,
                })
            })
            .collect(),
    )
}

/// I2C device `addr` on the RGB port of one GPU.
pub struct NvI2c {
    handle: usize,
    addr: u8,
}

impl NvI2c {
    pub fn new(gpu: &GpuIds, addr: u8) -> Self {
        Self {
            handle: gpu.handle,
            addr,
        }
    }

    fn transfer(&self, read: bool, reg: u8, data: &mut [u8]) -> io::Result<()> {
        let api = api().ok_or_else(|| io::Error::other("NvAPI unavailable"))?;
        let mut reg = reg;
        let mut info = I2cInfoV3 {
            version: (3 << 16) | std::mem::size_of::<I2cInfoV3>() as u32,
            display_mask: 0,
            is_ddc_port: 0,
            i2c_dev_address: self.addr << 1,
            i2c_reg_address: &mut reg,
            reg_addr_size: 1,
            data: data.as_mut_ptr(),
            size: data.len() as u32,
            i2c_speed: 0xFFFF,
            i2c_speed_khz: 0,
            port_id: RGB_PORT,
            is_port_id_set: 1,
        };
        let mut unknown: u32 = 0;
        let _guard = BUS_LOCK
            .lock()
            .map_err(|_| io::Error::other("NvAPI lock poisoned"))?;
        let f = if read { api.read } else { api.write };
        // SAFETY: `info` points at live locals for the duration of the call.
        let rc = unsafe { f(self.handle as Handle, &mut info, &mut unknown) };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!("NvAPI I2C status {rc}")))
        }
    }
}

impl GpuI2cBus for NvI2c {
    fn read_byte(&mut self, reg: u8) -> io::Result<u8> {
        let mut b = [0u8];
        self.transfer(true, reg, &mut b)?;
        Ok(b[0])
    }

    fn write_byte(&mut self, reg: u8, value: u8) -> io::Result<()> {
        self.transfer(false, reg, &mut [value])
    }

    fn write_block(&mut self, reg: u8, data: &[u8]) -> io::Result<()> {
        let mut buf = data.to_vec();
        self.transfer(false, reg, &mut buf)
    }
}
