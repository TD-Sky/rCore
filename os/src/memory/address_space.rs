//! 在内存分页的体系里，虚拟内存到物理内存天然就是恒等映射的。
//!
//! ## 平滑过渡
//!
//! 切换satp的指令，与其下一条指令，两者的虚拟地址是相邻的，
//! 它们的物理地址也是相邻的。
//! 因为切换stap的指令不属于跳转指令，PC只是简单地自增当前指令的字长。
//! 没有跳转地址，地址空间发生切换，页表就不能把连续的虚拟地址映射到
//! 不连续的物理地址上，故而我们前后两条指令都采用恒等映射。
//!
//! 在产生Trap前后的一小段时间内会出现**极端**情况：
//! 刚产生Trap时，CPU已经进入了内核态(S)，但此时执行代码和访问数据
//! 还是在用户空间内。此时跳板起了作用：对于所有地址空间，跳板的虚拟地址
//! 均一致，且均映射到同一物理地址，故而让内核态下处于用户空间运行变得安全。
//! 尔后，`__alltraps`会切换至内核空间。

use super::address::*;
use super::frame_allocator;
use super::frame_allocator::Frame;
use super::page_table;
use super::page_table::PTEFlag;
use super::page_table::{MappedVpn, UnmappedVpn};
use super::PageTable;

use crate::config::{MEMORY_END, MMIO, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_SIZE};
use crate::sync::UPSafeCell;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::riscv64;
use core::ops::Range;

use enumflags2::{bitflags, BitFlags};
use goblin::elf::Elf;
use goblin::elf64::program_header::PT_LOAD;
use goblin::elf64::program_header::{PF_R, PF_W, PF_X};
use lazy_static::lazy_static;
use log::{debug, info};
use riscv::register::satp;

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

lazy_static! {
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<AddressSpace>> =
        Arc::new(unsafe { UPSafeCell::new(AddressSpace::new_kernel()) });
}

/// 地址空间由一系列有关联但不一定连续的逻辑段组成
#[derive(Default)]
pub struct AddressSpace {
    page_table: PageTable,
    logic_segments: Vec<LogicSegment>,
}

struct LogicSegment {
    vpn_range: Range<VirtPageNum>,
    vpn2frame: BTreeMap<VirtPageNum, Frame>,
    map_type: MapType,
    permission: BitFlags<MapPermission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// 恒等映射
    Identical,

    /// 由分配器分配物理页的映射
    Framed,

    /// mmap直接映射
    Mmap,
}

/// 从页表项的标志位截出部分位，
/// 表示映射的权限
#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum MapPermission {
    R = 0b00000010,
    W = 0b00000100,
    X = 0b00001000,
    U = 0b00010000,
}

impl Clone for AddressSpace {
    /// 页表的内容是丢在内存里的，
    /// 类型级别的复制无法包含页表，
    /// 因此需要重新建立映射
    fn clone(&self) -> Self {
        // 用户地址空间的 fork
        let mut addr_space = Self::default();

        addr_space.map_trampoline();

        // 复制所有段
        for seg in &self.logic_segments {
            // 页表创建新的映射
            addr_space.push(seg.clone()).unwrap();
            // 取得物理页号，凭此复制该段的物理页
            for vpn in seg.vpn_range.clone() {
                let src_ppn = self.translate(vpn).unwrap().ppn();
                let dest_ppn = addr_space.translate(vpn).unwrap().ppn();
                dest_ppn
                    .page_bytes_mut()
                    .copy_from_slice(src_ppn.page_bytes());
            }
        }

        addr_space
    }
}

impl AddressSpace {
    /// 创建内核的地址空间
    ///
    /// NOTE: SV39分页模式下，64位的地址只有低39位是真正有意义的。
    /// SV39 分页模式规定 [39:63] 这25位必须与第38位一致，
    /// 否则MMU会直接认定它是一个不合法的虚拟地址。
    /// 因此，2^64个虚拟地址中，只有最高256G（前导1）与最低256G（前导0）可用。
    pub fn new_kernel() -> Self {
        debug!("creating kernel address space");
        let mut addr_space = Self::default();

        addr_space.map_trampoline();

        info!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        info!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        info!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        info!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );

        debug!("mapping .text section");
        addr_space
            .push(LogicSegment::new(
                stext as usize,
                etext as usize,
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ))
            .unwrap();

        debug!("mapping .rodata section");
        addr_space
            .push(LogicSegment::new(
                srodata as usize,
                erodata as usize,
                MapType::Identical,
                MapPermission::R.into(),
            ))
            .unwrap();

        debug!("mapping .data section");
        addr_space
            .push(LogicSegment::new(
                sdata as usize,
                edata as usize,
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ))
            .unwrap();

        debug!("mapping .bss section");
        addr_space
            .push(LogicSegment::new(
                sbss_with_stack as usize,
                ebss as usize,
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ))
            .unwrap();

        // 用户可用内存，交给物理页帧分配器
        debug!("mapping physical memory");
        addr_space
            .push(LogicSegment::new(
                ekernel as usize,
                MEMORY_END,
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ))
            .unwrap();

        debug!("mapping memory-mapped registers");
        for &(start, offset) in MMIO {
            addr_space
                .push(LogicSegment::new(
                    start,
                    start + offset,
                    MapType::Identical,
                    MapPermission::R | MapPermission::W,
                ))
                .unwrap();
        }

        addr_space
    }

    /// 创建用户的虚拟空间
    ///
    /// 返回：(地址空间, 用户栈顶地址, 程序入口地址)
    pub fn new_user(elf_data: &[u8]) -> (Self, usize, usize) {
        debug!("creating user address space");
        let mut addr_space = Self::default();

        addr_space.map_trampoline();

        let elf = Elf::parse(elf_data).unwrap();

        // 魔数，ELF头的首串字节，用于核对文件是否为ELF
        let magic = &elf.header.e_ident[0..4];
        assert_eq!(magic, &[0x7f, 0x45, 0x4c, 0x46], "invalid elf!");

        // 所有段分配完空间后，最后之段的末页号
        let mut max_end_vpn = VirtPageNum::default();

        // 为LOAD类型的段映射空间，并加载至内存中
        for ph in elf.program_headers.iter().filter(|ph| ph.p_type == PT_LOAD) {
            let start_va = VirtAddr::from(ph.p_vaddr as usize);
            let end_va = VirtAddr::from((ph.p_vaddr + ph.p_memsz) as usize);

            let mut permission = BitFlags::from(MapPermission::U);
            let ph_flags = ph.p_flags;
            if (ph_flags & PF_R) == PF_R {
                permission |= MapPermission::R;
            }
            if (ph_flags & PF_W) == PF_W {
                permission |= MapPermission::W;
            }
            if (ph_flags & PF_X) == PF_X {
                permission |= MapPermission::X;
            }

            let seg = LogicSegment::new(start_va, end_va, MapType::Framed, permission);

            max_end_vpn = seg.vpn_range.end;

            addr_space
                .push_with_data(
                    seg,
                    &elf_data[ph.p_offset as usize..((ph.p_offset + ph.p_filesz) as usize)],
                )
                .unwrap();
        }

        let max_end_vpn: usize = VirtAddr::from(max_end_vpn).into();
        let user_stack_bottom = max_end_vpn + PAGE_SIZE; // 这样就空出了一个保护页
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;

        // 用户栈
        addr_space
            .push(LogicSegment::new(
                user_stack_bottom,
                user_stack_top,
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ))
            .unwrap();

        // 堆，虽然此处是零分配，但令地址空间创建了堆逻辑段
        addr_space
            .push(LogicSegment::new(
                user_stack_top,
                user_stack_top,
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ))
            .unwrap();

        // 映射Trap上下文的空间
        addr_space
            .push(LogicSegment::new(
                TRAP_CONTEXT,
                TRAMPOLINE,
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ))
            .unwrap();

        (addr_space, user_stack_top, elf.header.e_entry as usize)
    }

    pub fn insert_framed(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: BitFlags<MapPermission>,
    ) -> Result<(), MappedVpn> {
        self.push(LogicSegment::new(
            start_va,
            end_va,
            MapType::Framed,
            permission,
        ))
    }

    /// 映射一块内存
    ///
    /// suggested_start 建议的地址：
    /// - 若用户尚未给出建议，则该地址为默认的**mmap起始地址**；
    /// - 若用户给出建议地址，本函数的调用者应该确认其不低于mmap起始地址，否则用默认的代替。
    ///
    /// 总之，建议地址一定大于等于mmap起始地址。
    pub fn insert_mmap(
        &mut self,
        suggested_start: VirtAddr,
        len: usize,
        permission: BitFlags<MapPermission>,
    ) -> Result<VirtAddr, MappedVpn> {
        let actual_start = self
            .logic_segments
            .iter()
            .filter(|&seg| seg.map_type == MapType::Mmap)
            .map(|seg| &seg.vpn_range)
            .fold(
                suggested_start,
                |actual_start: VirtAddr, &Range { start, end }| {
                    let actual_start_vpn = actual_start.page_number();

                    if (actual_start_vpn < start
                        && (actual_start + len + PAGE_SIZE).page_number() >= start)
                        || (start <= actual_start_vpn && actual_start_vpn <= end)
                    {
                        // 不合格的情况：
                        // - 将要映射的逻辑段(前)(计入保护页)与已映射段(后)交叠；
                        // - 将要映射的逻辑段开始地址落在已映射段内(包含结束地址)；
                        //
                        // 取末页加上保护页作为新的起始地址
                        VirtAddr::from(end + 1)
                    } else {
                        // 合格
                        actual_start
                    }
                },
            );

        debug!(
            "area of mmap: [{:#x}, {:#x}) ",
            usize::from(actual_start),
            usize::from(actual_start + len)
        );

        self.push(LogicSegment::new(
            actual_start,
            actual_start + len,
            MapType::Mmap,
            permission,
        ))?;

        Ok(actual_start)
    }

    pub fn remove(&mut self, start: VirtPageNum) -> Result<(), MapError> {
        let Some(index) = self
            .logic_segments
            .iter_mut()
            .position(|seg| seg.vpn_range.start == start)
        else {
            return Err(MapError {
                vpn: start,
                kind: MapErrorKind::NoSegement,
            });
        };

        let mut seg = self.logic_segments.remove(index);
        seg.unmap(&mut self.page_table)?;

        Ok(())
    }

    pub fn remove_mmap(&mut self, start: VirtPageNum) -> Result<(), MapError> {
        let Some(index) = self
            .logic_segments
            .iter_mut()
            .position(|seg| seg.vpn_range.start == start)
        else {
            return Err(MapError {
                vpn: start,
                kind: MapErrorKind::NoSegement,
            });
        };

        if self.logic_segments[index].map_type != MapType::Mmap {
            return Err(MapError {
                vpn: start,
                kind: MapErrorKind::TypeMissed,
            });
        }

        let mut seg = self.logic_segments.remove(index);
        seg.unmap(&mut self.page_table)?;

        Ok(())
    }

    /// 删除所有段，主要目的是归还物理页帧
    pub fn clear(&mut self) {
        self.logic_segments.clear();
    }

    pub fn translate(&self, vpn: impl Into<VirtPageNum>) -> Option<&page_table::Entry> {
        self.page_table.translate(vpn.into())
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    pub fn activate(&self) {
        let satp = self.page_table.token();
        satp::write(satp);
        // 快表 TLB 会缓存虚拟页号到页表项的映射，
        // 但它与地址空间耦合，
        // 改动 satp 的值即切换地址空间会导致
        // 快表缓存失效，故而需要清零
        unsafe { riscv64::sfence_vma_all() };
    }

    pub fn shrink_to(&mut self, start: VirtAddr, lower_end: VirtAddr) -> Result<(), MapError> {
        match self
            .logic_segments
            .iter_mut()
            .find(|seg| seg.vpn_range.start == start.page_number())
        {
            Some(seg) => Ok(seg.shrink_to(&mut self.page_table, lower_end.page_number())?),
            None => Err(MapError {
                vpn: start.page_number(),
                kind: MapErrorKind::NoSegement,
            }),
        }
    }

    pub fn expand_to(&mut self, start: VirtAddr, higher_end: VirtAddr) -> Result<(), MapError> {
        match self
            .logic_segments
            .iter_mut()
            .find(|seg| seg.vpn_range.start == start.page_number())
        {
            Some(seg) => Ok(seg.expand_to(&mut self.page_table, higher_end.page_number())?),
            None => Err(MapError {
                vpn: start.page_number(),
                kind: MapErrorKind::NoSegement,
            }),
        }
    }
}

impl AddressSpace {
    /// 在内核地址空间的高256G部分之顶层分配跳板。
    /// 实际上是将虚拟地址 TRAMPOLINE 映射到 .text.strampoline
    fn map_trampoline(&mut self) {
        self.page_table
            .map(
                VirtAddr::from(TRAMPOLINE),
                PhysAddr::from(strampoline as usize),
                PTEFlag::R | PTEFlag::X,
            )
            .unwrap();
    }

    fn push(&mut self, mut seg: LogicSegment) -> Result<(), MappedVpn> {
        seg.map(&mut self.page_table)?;
        self.logic_segments.push(seg);
        Ok(())
    }

    fn push_with_data(&mut self, mut seg: LogicSegment, data: &[u8]) -> Result<(), MappedVpn> {
        seg.map(&mut self.page_table)?;
        seg.write_data(&self.page_table, data);
        self.logic_segments.push(seg);
        Ok(())
    }
}

impl Clone for LogicSegment {
    fn clone(&self) -> Self {
        // fork 出来的逻辑段不真正映射到物理页帧上
        Self {
            vpn_range: self.vpn_range.clone(),
            vpn2frame: BTreeMap::new(),
            map_type: self.map_type,
            permission: self.permission,
        }
    }
}

impl LogicSegment {
    fn new<V: Into<VirtAddr>>(
        start_va: V,
        end_va: V,
        map_type: MapType,
        permission: BitFlags<MapPermission>,
    ) -> Self {
        let start = start_va.into().floor();
        let end = end_va.into().ceil();
        Self {
            vpn_range: Range { start, end },
            vpn2frame: BTreeMap::new(),
            map_type,
            permission,
        }
    }

    /// 将该逻辑段映射到物理内存
    fn map(&mut self, page_table: &mut PageTable) -> Result<(), MappedVpn> {
        for vpn in self.vpn_range.clone() {
            // 若VPN已被映射，则回收该逻辑段已分配的内存
            if let Err(e) = self.map_one(page_table, vpn) {
                self.vpn_range.end = vpn;
                self.unmap(page_table).unwrap();
                return Err(e);
            }
        }

        Ok(())
    }

    /// 取消该逻辑段对物理内存的映射
    fn unmap(&mut self, page_table: &mut PageTable) -> Result<(), UnmappedVpn> {
        for vpn in self.vpn_range.clone() {
            self.unmap_one(page_table, vpn)?;
        }

        Ok(())
    }

    fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) -> Result<(), MappedVpn> {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => ppn = vpn.identity_map(),
            // 暂时让Mmap与Framed等价
            MapType::Framed | MapType::Mmap => {
                let frame = frame_allocator::alloc().unwrap();
                ppn = frame.ppn;
                self.vpn2frame.insert(vpn, frame);
            }
        }

        let pte_flags = BitFlags::from_bits_truncate(self.permission.bits());
        page_table.map(vpn, ppn, pte_flags)
    }

    fn unmap_one(
        &mut self,
        page_table: &mut PageTable,
        vpn: VirtPageNum,
    ) -> Result<(), UnmappedVpn> {
        if self.map_type == MapType::Framed {
            self.vpn2frame.remove(&vpn);
        }

        page_table.unmap(vpn)
    }

    /// 将数据写到逻辑段所映射的物理页内
    fn write_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);

        let len = data.len();

        for (start, current_vpn) in (0..len).step_by(PAGE_SIZE).zip(self.vpn_range.clone()) {
            let end = len.min(start + PAGE_SIZE);
            let src = &data[start..end];

            let ppn = page_table.translate(current_vpn).unwrap().ppn();
            let dst = &mut ppn.page_bytes_mut()[..src.len()];

            dst.copy_from_slice(src);
        }
    }

    fn shrink_to(
        &mut self,
        page_table: &mut PageTable,
        lower_end: VirtPageNum,
    ) -> Result<(), UnmappedVpn> {
        for vpn in lower_end..self.vpn_range.end {
            self.unmap_one(page_table, vpn)?;
        }

        self.vpn_range.end = lower_end;
        Ok(())
    }

    fn expand_to(
        &mut self,
        page_table: &mut PageTable,
        higher_end: VirtPageNum,
    ) -> Result<(), MappedVpn> {
        for vpn in self.vpn_range.end..higher_end {
            self.map_one(page_table, vpn)?;
        }

        self.vpn_range.end = higher_end;
        Ok(())
    }
}

pub use error::*;
mod error {
    use crate::memory::address::VirtPageNum;
    use crate::memory::page_table::{MappedVpn, UnmappedVpn};

    #[derive(Debug)]
    pub struct MapError {
        pub vpn: VirtPageNum,
        pub kind: MapErrorKind,
    }

    #[derive(Debug)]
    pub enum MapErrorKind {
        NoSegement,
        MappedVpn,
        UnmappedVpn,
        TypeMissed,
    }

    impl From<MappedVpn> for MapError {
        fn from(MappedVpn(vpn): MappedVpn) -> Self {
            Self {
                vpn,
                kind: MapErrorKind::MappedVpn,
            }
        }
    }

    impl From<UnmappedVpn> for MapError {
        fn from(UnmappedVpn(vpn): UnmappedVpn) -> Self {
            Self {
                vpn,
                kind: MapErrorKind::UnmappedVpn,
            }
        }
    }
}
