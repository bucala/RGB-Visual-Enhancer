//! Minimal SMBus abstraction.
//!
//! * **Linux:** `/dev/i2c-N` via the `I2C_SMBUS` ioctl (needs the `i2c-dev`
//!   module and read/write access to the node).
//! * **Windows:** SMBus host controllers are only reachable from kernel mode
//!   (Ring0). A backend for a signed port-I/O driver (e.g. PawnIO) can
//!   implement [`SmbusBus`]; none is bundled yet, so detection reports that.

use std::io;

/// Synchronous SMBus transactions. Always called from `spawn_blocking`.
pub trait SmbusBus: Send {
    fn name(&self) -> &str;
    /// SMBus "receive byte" – used as a presence probe.
    fn read_byte(&mut self, addr: u8) -> io::Result<u8>;
    fn read_byte_data(&mut self, addr: u8, cmd: u8) -> io::Result<u8>;
    fn write_byte_data(&mut self, addr: u8, cmd: u8, value: u8) -> io::Result<()>;
    fn write_word_data(&mut self, addr: u8, cmd: u8, value: u16) -> io::Result<()>;
    /// SMBus block write (length-prefixed, max 32 bytes).
    fn write_block_data(&mut self, addr: u8, cmd: u8, data: &[u8]) -> io::Result<()>;
}

pub const SMBUS_BLOCK_MAX: usize = 32;

/// A candidate bus discovered on the system.
pub struct BusDescriptor {
    pub name: String,
    pub open: Box<dyn FnOnce() -> io::Result<Box<dyn SmbusBus>> + Send>,
    /// PCI subsystem vendor of the host controller, when known. ASUS boards
    /// report 0x1043 here, which gates probing of motherboard addresses.
    pub subsystem_vendor: Option<u16>,
}

#[cfg(target_os = "linux")]
pub use linux::enumerate_buses;

#[cfg(not(target_os = "linux"))]
pub fn enumerate_buses() -> Result<Vec<BusDescriptor>, String> {
    Err("direct SMBus access on this OS requires a Ring0 driver (e.g. PawnIO), which is not bundled yet".into())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs::{self, File, OpenOptions};
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    const I2C_SLAVE: libc::c_ulong = 0x0703;
    const I2C_SMBUS: libc::c_ulong = 0x0720;
    const I2C_SMBUS_READ: u8 = 1;
    const I2C_SMBUS_WRITE: u8 = 0;
    const I2C_SMBUS_BYTE: u32 = 1;
    const I2C_SMBUS_BYTE_DATA: u32 = 2;
    const I2C_SMBUS_WORD_DATA: u32 = 3;
    const I2C_SMBUS_BLOCK_DATA: u32 = 5;

    /// `union i2c_smbus_data { __u8 byte; __u16 word; __u8 block[34]; }`
    #[repr(C)]
    union SmbusData {
        byte: u8,
        word: u16,
        block: [u8; SMBUS_BLOCK_MAX + 2],
    }

    /// `struct i2c_smbus_ioctl_data`
    #[repr(C)]
    struct SmbusIoctlData {
        read_write: u8,
        command: u8,
        size: u32,
        data: *mut SmbusData,
    }

    pub struct LinuxSmbus {
        name: String,
        file: File,
        current_addr: Option<u8>,
    }

    impl LinuxSmbus {
        pub fn open(dev: &Path, name: String) -> io::Result<Self> {
            let file = OpenOptions::new().read(true).write(true).open(dev)?;
            Ok(Self {
                name,
                file,
                current_addr: None,
            })
        }

        fn select(&mut self, addr: u8) -> io::Result<()> {
            if self.current_addr == Some(addr) {
                return Ok(());
            }
            // SAFETY: I2C_SLAVE takes the 7-bit address by value.
            let rc = unsafe {
                libc::ioctl(
                    self.file.as_raw_fd(),
                    I2C_SLAVE as _,
                    libc::c_ulong::from(addr),
                )
            };
            if rc < 0 {
                return Err(io::Error::last_os_error());
            }
            self.current_addr = Some(addr);
            Ok(())
        }

        fn access(
            &mut self,
            addr: u8,
            rw: u8,
            cmd: u8,
            size: u32,
            data: &mut SmbusData,
        ) -> io::Result<()> {
            self.select(addr)?;
            let mut args = SmbusIoctlData {
                read_write: rw,
                command: cmd,
                size,
                data: data as *mut _,
            };
            // SAFETY: `args` and `data` are valid for the duration of the
            // call and match the kernel's ABI layouts.
            let rc = unsafe {
                libc::ioctl(
                    self.file.as_raw_fd(),
                    I2C_SMBUS as _,
                    &mut args as *mut SmbusIoctlData,
                )
            };
            if rc < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    impl SmbusBus for LinuxSmbus {
        fn name(&self) -> &str {
            &self.name
        }

        fn read_byte(&mut self, addr: u8) -> io::Result<u8> {
            let mut d = SmbusData { block: [0; 34] };
            self.access(addr, I2C_SMBUS_READ, 0, I2C_SMBUS_BYTE, &mut d)?;
            // SAFETY: the kernel filled the `byte` member.
            Ok(unsafe { d.byte })
        }

        fn read_byte_data(&mut self, addr: u8, cmd: u8) -> io::Result<u8> {
            let mut d = SmbusData { block: [0; 34] };
            self.access(addr, I2C_SMBUS_READ, cmd, I2C_SMBUS_BYTE_DATA, &mut d)?;
            // SAFETY: as above.
            Ok(unsafe { d.byte })
        }

        fn write_byte_data(&mut self, addr: u8, cmd: u8, value: u8) -> io::Result<()> {
            let mut d = SmbusData { byte: value };
            self.access(addr, I2C_SMBUS_WRITE, cmd, I2C_SMBUS_BYTE_DATA, &mut d)
        }

        fn write_word_data(&mut self, addr: u8, cmd: u8, value: u16) -> io::Result<()> {
            let mut d = SmbusData { word: value };
            self.access(addr, I2C_SMBUS_WRITE, cmd, I2C_SMBUS_WORD_DATA, &mut d)
        }

        fn write_block_data(&mut self, addr: u8, cmd: u8, data: &[u8]) -> io::Result<()> {
            if data.is_empty() || data.len() > SMBUS_BLOCK_MAX {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "SMBus block must be 1..=32 bytes",
                ));
            }
            let mut block = [0u8; 34];
            block[0] = data.len() as u8;
            block[1..=data.len()].copy_from_slice(data);
            let mut d = SmbusData { block };
            self.access(addr, I2C_SMBUS_WRITE, cmd, I2C_SMBUS_BLOCK_DATA, &mut d)
        }
    }

    fn read_trim(p: &Path) -> Option<String> {
        fs::read_to_string(p).ok().map(|s| s.trim().to_string())
    }

    /// Chipset SMBus adapters (Intel i801, AMD PIIX4/FCH). Other adapters
    /// (GPU DDC, display buses) are deliberately ignored.
    pub fn enumerate_buses() -> Result<Vec<BusDescriptor>, String> {
        let root = Path::new("/sys/bus/i2c/devices");
        let entries =
            fs::read_dir(root).map_err(|e| format!("cannot list {}: {e}", root.display()))?;
        let mut buses = Vec::new();
        let mut missing_dev_node = false;
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Some(num) = file_name.strip_prefix("i2c-") else {
                continue;
            };
            let sys = entry.path();
            let Some(name) = read_trim(&sys.join("name")) else {
                continue;
            };
            if !(name.contains("SMBus I801") || name.contains("SMBus PIIX4")) {
                continue;
            }
            let dev = Path::new("/dev").join(&file_name);
            if !dev.exists() {
                missing_dev_node = true;
                continue;
            }
            let subsystem_vendor = read_trim(&sys.join("device/subsystem_vendor"))
                .and_then(|v| u16::from_str_radix(v.trim_start_matches("0x"), 16).ok());
            let label = format!("i2c-{num} ({name})");
            let label_c = label.clone();
            buses.push(BusDescriptor {
                name: label,
                subsystem_vendor,
                open: Box::new(move || {
                    LinuxSmbus::open(&dev, label_c).map(|b| Box::new(b) as Box<dyn SmbusBus>)
                }),
            });
        }
        if buses.is_empty() {
            return Err(if missing_dev_node {
                "chipset SMBus found but /dev/i2c-* is missing: run `sudo modprobe i2c-dev`".into()
            } else {
                "no chipset SMBus adapter found (is the i2c-i801 / i2c-piix4 driver loaded?)".into()
            });
        }
        Ok(buses)
    }
}
